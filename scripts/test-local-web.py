#!/usr/bin/env python3
"""Isolated end-to-end smoke test for an already-built QinAegis executable.

Usage: python3 scripts/test-local-web.py /absolute/path/to/qinaegis
Uses only the standard library. No browser, engine, model or user data is used.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import http.client
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import sys
import tempfile
import time
from http.cookies import SimpleCookie
from urllib.parse import urlsplit


class SmokeFailure(Exception):
    pass


def require(condition, message):
    if not condition:
        raise SmokeFailure(message)


class Smoke:
    def __init__(self, binary, directory):
        self.binary = binary
        root = Path(directory)
        self.home = root / "home"
        self.state = root / "state"
        self.home.mkdir()
        self.env = os.environ.copy()
        self.env.update({
            "HOME": str(self.home), "USERPROFILE": str(self.home),
            "XDG_CONFIG_HOME": str(self.home / "config"),
            "APPDATA": str(self.home / "appdata"),
            "LOCALAPPDATA": str(self.home / "localappdata"),
            "QINAEGIS_STATE_DIR": str(self.state),
        })
        self.instance = None
        self.authorized = None
        self.rpc_sequence = 0
        self.secrets = []
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
            sock.bind(("127.0.0.1", 0))
            self.port = sock.getsockname()[1]
        self.origin = f"http://127.0.0.1:{self.port}"

    def cli(self, *arguments, success=True):
        try:
            result = subprocess.run(
                [str(self.binary), *arguments], env=self.env,
                cwd=self.home, capture_output=True, text=True, timeout=20,
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            raise SmokeFailure(f"CLI {arguments[0]} could not complete ({type(error).__name__})") from None
        # Do not include subprocess output in errors: future launchers may emit credentials.
        require((result.returncode == 0) == success,
                f"CLI {arguments[0]} returned unexpected exit status {result.returncode}")
        return result

    def read_instance(self):
        try:
            instance = json.loads((self.state / "instance.json").read_text())
        except (OSError, ValueError):
            raise SmokeFailure("isolated service instance record unavailable") from None
        require(instance.get("origin") == self.origin, "instance used unexpected origin")
        require(isinstance(instance.get("pid"), int) and instance["pid"] > 0,
                "instance has invalid process identity")
        require(isinstance(instance.get("instance_id"), str) and
                isinstance(instance.get("owner_secret"), str), "instance credentials unavailable")
        return instance

    def request(self, method, path, body=None, headers=None, expected=200):
        request_headers = dict(headers or {})
        data = None
        if body is not None:
            data = json.dumps(body).encode()
            request_headers.setdefault("Content-Type", "application/json")
        conn = http.client.HTTPConnection("127.0.0.1", self.port, timeout=5)
        try:
            conn.request(method, path, body=data, headers=request_headers)
            response = conn.getresponse()
            response_headers = dict(response.getheaders())
            payload = response.read()
            require(all(secret.encode() not in payload for secret in self.secrets),
                    "service response exposed a model credential")
            require(response.status == expected,
                    f"HTTP {method} {path} expected {expected}, received {response.status}")
            return payload, response_headers
        except (OSError, http.client.HTTPException) as error:
            raise SmokeFailure(f"HTTP {method} {path} failed ({type(error).__name__})") from None
        finally:
            conn.close()

    def json_request(self, *arguments, **kwargs):
        payload, headers = self.request(*arguments, **kwargs)
        try:
            return json.loads(payload), headers
        except ValueError:
            raise SmokeFailure("service returned invalid JSON") from None

    def rpc(self, method, params=None, success=True):
        self.rpc_sequence += 1
        request_id = f"smoke-{self.rpc_sequence}"
        response, _ = self.json_request("POST", "/api/v2/rpc",
            body={"id": request_id, "method": method, "params": params or {}},
            headers=self.authorized)
        require(response.get("id") == request_id and response.get("ok") is success,
                f"RPC {method} returned unexpected result")
        if not success:
            require(isinstance(response.get("error"), dict), f"RPC {method} omitted error envelope")
            return response["error"]
        require("data" in response, f"RPC {method} omitted data")
        return response["data"]

    def authenticate(self):
        launched, _ = self.json_request("POST", "/api/v2/launch", body={},
            headers={"x-qinaegis-owner": self.instance["owner_secret"]})
        session, headers = self.json_request("POST", "/api/v2/session",
            body={"token": launched.get("token")}, headers={"Origin": self.origin})
        cookie = SimpleCookie()
        cookie.load(headers.get("set-cookie", headers.get("Set-Cookie", "")))
        require("qinaegis_session" in cookie and bool(session.get("csrf")),
                "restarted service authentication failed")
        self.authorized = {
            "Cookie": f"qinaegis_session={cookie['qinaegis_session'].value}",
            "Origin": self.origin, "x-qinaegis-csrf": session["csrf"],
        }

    def exercise_contracts(self):
        for project in ("smoke-alpha", "smoke-beta"):
            self.rpc("createProject", {"name": project, "url": f"https://{project}.invalid",
                                       "tech_stack": []})
        empty = self.rpc("getContracts")
        require(empty.get("revision") == 0 and empty.get("publications") == [],
                "fresh contract library is not empty")
        draft = {"id": "receipt", "version": 1, "intent": "submission returns a receipt",
                 "inputs": {"target": "string", "quantity": "number"}, "outputs": {},
                 "preconditions": [], "assertions": ["receipt matches submitted input"],
                 "required_capabilities": ["browser.interact"]}
        saved = self.rpc("saveContractDraft", {"draft": draft, "expected_revision": 0})
        require(saved.get("revision") == 1, "draft save did not advance revision")
        self.rpc("publishContract", {"id": "receipt", "expected_revision": 1}, success=False)
        require(self.rpc("getContracts") == saved, "unconfirmed publication changed library")
        published = self.rpc("publishContract", {"id": "receipt", "expected_revision": 1, "approved": True})
        original = published["publications"][0]
        require(original.get("reviewer") == "local-owner", "publication review identity missing")
        alpha_inputs = {"target": "https://alpha-project-input.invalid", "quantity": 2}
        beta_inputs = {"target": "https://beta-project-input.invalid", "quantity": 7}
        alpha = self.rpc("bindContract", {"project": "smoke-alpha", "contract_id": "receipt",
            "version": 1, "parameters": alpha_inputs, "expected_revision": 2})
        self.rpc("bindContract", {"project": "smoke-beta", "contract_id": "receipt", "version": 1,
            "parameters": {"target": 42, "quantity": "wrong"}, "expected_revision": 3}, success=False)
        require(self.rpc("getContracts") == alpha, "invalid typed inputs changed library")
        bound = self.rpc("bindContract", {"project": "smoke-beta", "contract_id": "receipt",
            "version": 1, "parameters": beta_inputs, "expected_revision": 3})
        parameters = {b["project"]: b["parameters"] for b in bound["bindings"]}
        require(parameters == {"smoke-alpha": alpha_inputs, "smoke-beta": beta_inputs},
                "project bindings did not retain independent typed parameters")
        replacement = {**draft, "version": 2,
                       "assertions": draft["assertions"] + ["receipt has a timestamp"]}
        self.rpc("saveContractDraft", {"draft": replacement, "expected_revision": 3}, success=False)
        require(self.rpc("getContracts") == bound, "stale CAS update changed contract library")
        proposed = self.rpc("proposeContractCorrection", {"contract_id": "receipt", "version": 1,
            "replacement": replacement, "reason": "clarify receipt evidence",
            "evidence": ["smoke-observation/1"], "expected_revision": 4})
        proposal_id = proposed["proposals"][0]["id"]
        review = {"proposal_id": proposal_id, "decision": "approve", "expected_revision": 5}
        self.rpc("reviewContractCorrection", review, success=False)
        require(self.rpc("getContracts") == proposed, "unconfirmed correction changed library")
        corrected = self.rpc("reviewContractCorrection", {**review, "approved": True})
        require(corrected.get("revision") == 6 and corrected["publications"][0] == original
                and corrected["publications"][1]["definition"]["version"] == 2,
                "correction did not retain immutable publication history")
        require(corrected["proposals"][0]["status"] == "approved"
                and all(b["contract_version"] == 1 for b in corrected["bindings"])
                and corrected["bindings"] == bound["bindings"],
                "correction silently upgraded project bindings")
        exported = self.rpc("exportContracts")
        require(set(exported) == {"schema_version", "contracts"}
                and exported["contracts"] == corrected["publications"], "contract export schema unexpected")
        export_text = json.dumps(exported)
        require(all(value["target"] not in export_text for value in (alpha_inputs, beta_inputs))
                and "parameters" not in export_text, "contract export leaked project parameters")
        print("PASS contract draft, explicit review, typed project reuse, CAS, immutable correction and safe export")
        return corrected

    def exercise_models(self):
        empty = self.rpc("getModels")
        require(empty.get("revision") == 0 and empty.get("profiles") == [], "fresh model registry is not empty")
        key = "smoke-only-" + os.urandom(24).hex()
        self.secrets.append(key)
        profile = {"id": "smoke-model", "label": "Isolated test model", "adapter": "openai_compatible",
                   "base_url": "https://model-smoke.invalid/v1", "model": "smoke-text",
                   "api_key": key, "capabilities": ["text"], "enabled": True,
                   "allow_remote": True, "midscene_family": None}
        saved = self.rpc("saveModel", {"profile": profile, "expected_revision": 0})
        require(saved.get("revision") == 1 and len(saved.get("profiles", [])) == 1,
                "model registration failed")
        redacted = saved["profiles"][0]
        require(redacted.get("api_key") == "" and redacted.get("api_key_configured") is True,
                "model credential was not redacted with configured state")
        require(self.rpc("getModels") == saved, "model read changed redacted profile")
        self.rpc("saveModel", {"profile": profile, "expected_revision": 0}, success=False)
        self.rpc("saveModel", {"profile": {**profile, "base_url": f"https://user:{key}@model-smoke.invalid"},
                               "expected_revision": 1}, success=False)
        require(self.rpc("getModels") == saved, "invalid model save changed registry")
        routed = self.rpc("setModelRoute", {"task": "generate", "route": {"primary": "smoke-model", "fallbacks": []},
                                           "expected_revision": 1})
        require(routed.get("revision") == 2, "model route save did not advance revision")
        resolved = self.rpc("resolveModelRoute", {"task": "generate"})
        require(resolved.get("revision") == 2 and resolved.get("task") == "generate"
                and [p.get("id") for p in resolved.get("candidates", [])] == ["smoke-model"],
                "model route did not resolve pinned configured profile")
        self.rpc("resolveModelRoute", {"task": "execute"}, success=False)
        print("PASS model registration, route resolution, CAS and response/error credential redaction without model calls")
        return routed, resolved

    def run(self):
        self.cli("status", success=False)
        with ThreadPoolExecutor(max_workers=2) as launchers:
            starts = [launchers.submit(self.cli, "start", "--no-open", "--port", str(self.port))
                      for _ in range(2)]
            for started in starts:
                started.result()
        self.instance = self.read_instance()
        status = self.cli("status")
        require(f"PID {self.instance['pid']}" in status.stdout, "status did not identify service PID")
        self.cli("start", "--no-open", "--port", str(self.port))
        require(self.read_instance() == self.instance, "repeated start created a different instance")
        another_port = 1 if self.port == 65535 else self.port + 1
        self.cli("start", "--no-open", "--port", str(another_port), success=False)
        require(self.read_instance() == self.instance, "explicit port conflict changed instance")
        print("PASS concurrent background start, status, instance reuse and explicit port policy")

        health, _ = self.json_request("GET", "/healthz")
        require(health.get("product") == "qinaegis" and health.get("ready") is True
                and health.get("api") == 2 and health.get("instance_id") == self.instance["instance_id"],
                "health identity/readiness mismatch")
        html, _ = self.request("GET", "/")
        require(b"<html" in html.lower(), "root did not serve HTML")
        assets = re.findall(r'(?:src|href)=[\"\']([^\"\']+\.(?:js|css))[\"\']', html.decode())
        require(bool(assets), "production HTML has no compiled assets")
        for asset in assets:
            parsed = urlsplit(asset)
            require(not parsed.scheme and not parsed.netloc, "UI asset is not local")
            self.request("GET", "/" + asset.lstrip("./"))
        self.request("GET", "/missing-smoke-asset", expected=404)
        self.request("GET", "/healthz", headers={"Host": "host.invalid"}, expected=403)
        self.request("POST", "/api/v2/launch", body={}, expected=401)
        self.request("GET", "/api/v2/session", expected=401)
        print("PASS health, embedded assets, real 404 and Host/owner boundaries")

        launched, _ = self.json_request("POST", "/api/v2/launch", body={},
            headers={"x-qinaegis-owner": self.instance["owner_secret"]})
        token = launched.get("token")
        require(isinstance(token, str) and len(token) == 64, "owner launch token unavailable")
        session, headers = self.json_request("POST", "/api/v2/session", body={"token": token},
            headers={"Origin": self.origin})
        cookie = SimpleCookie()
        cookie.load(headers.get("set-cookie", headers.get("Set-Cookie", "")))
        require("qinaegis_session" in cookie, "session cookie unavailable")
        morsel = cookie["qinaegis_session"]
        require(bool(morsel["httponly"]) and morsel["samesite"].lower() == "strict",
                "session cookie security attributes missing")
        cookie_header = {"Cookie": f"qinaegis_session={morsel.value}"}
        self.request("POST", "/api/v2/session", body={"token": token},
            headers={"Origin": self.origin}, expected=401)
        refreshed, _ = self.json_request("GET", "/api/v2/session", headers=cookie_header)
        require(refreshed.get("csrf") == session.get("csrf") and bool(refreshed.get("csrf")),
                "session refresh did not restore CSRF token")
        authorized = {**cookie_header, "Origin": self.origin, "x-qinaegis-csrf": refreshed["csrf"]}
        self.authorized = authorized
        rpc_body = {"id": "smoke-config", "method": "checkConfig", "params": {}}
        rpc, _ = self.json_request("POST", "/api/v2/rpc", body=rpc_body, headers=authorized)
        require(rpc.get("ok") is True and rpc.get("id") == "smoke-config"
                and rpc.get("data", {}).get("configured") is False,
                "fresh isolated config RPC response unexpected")
        self.request("POST", "/api/v2/rpc", body=rpc_body,
            headers={**authorized, "Origin": "https://origin.invalid"}, expected=403)
        self.request("POST", "/api/v2/rpc", body=rpc_body,
            headers={**cookie_header, "Origin": self.origin}, expected=401)
        self.request("POST", "/api/v2/rpc", body=rpc_body,
            headers={**authorized, "x-qinaegis-csrf": "incorrect"}, expected=401)
        print("PASS one-use bootstrap, cookie refresh, config RPC and Origin/CSRF boundaries")

        contracts = self.exercise_contracts()
        models, route = self.exercise_models()

        old_instance_id = self.instance["instance_id"]
        self.cli("stop")
        self.cli("status", success=False)
        self.instance = None
        self.cli("start", "--no-open", "--port", str(self.port))
        self.instance = self.read_instance()
        require(self.instance["instance_id"] != old_instance_id, "restart reused old service identity")
        self.request("POST", "/api/v2/rpc", body=rpc_body, headers=authorized, expected=401)
        self.authenticate()
        require(self.rpc("getContracts") == contracts, "contract library did not survive restart")
        require(self.rpc("getModels") == models, "redacted model registry did not survive restart")
        require(self.rpc("resolveModelRoute", {"task": "generate"}) == route,
                "model route or configured credential did not survive restart")
        print("PASS restart persistence, fresh session boundary and model route reload")

        self.cli("stop")
        self.cli("status", success=False)
        self.instance = None
        print("PASS clean stop and stopped status")
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as occupied:
            occupied.bind(("127.0.0.1", 0))
            occupied.listen()
            occupied_port = occupied.getsockname()[1]
            self.cli("serve", "--no-open", "--port", str(occupied_port), success=False)
        require(not (self.state / "instance.json").exists(),
                "failed explicit-port bind published an instance")
        print("PASS occupied explicit port fails without publishing readiness")

    def cleanup(self):
        # Never use default state or kill by a potentially stale PID. Only issue
        # shutdown against this unique temporary directory and selected origin.
        if not (self.state / "instance.json").exists():
            return
        try:
            current = self.read_instance()
            if self.instance is not None:
                require(current == self.instance, "cleanup identity changed")
            health, _ = self.json_request("GET", "/healthz")
            require(health.get("instance_id") == current["instance_id"], "cleanup identity mismatch")
            self.json_request("POST", "/api/v2/shutdown", body={},
                headers={"x-qinaegis-owner": current["owner_secret"]})
            for _ in range(50):
                if not (self.state / "instance.json").exists():
                    return
                time.sleep(0.1)
            raise SmokeFailure("isolated service did not finish shutdown")
        except SmokeFailure:
            print("WARNING: isolated instance cleanup could not be confirmed; no unrelated process was killed", file=sys.stderr)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path, help="absolute path to already-built executable")
    args = parser.parse_args()
    if not (args.binary.is_absolute() and args.binary.is_file() and os.access(args.binary, os.X_OK)):
        parser.error("binary must be an absolute executable file")
    with tempfile.TemporaryDirectory(prefix="qinaegis-web-smoke-") as directory:
        smoke = Smoke(args.binary.resolve(), directory)
        try:
            smoke.run()
        except SmokeFailure as error:
            print(f"FAIL {error}", file=sys.stderr)
            return 1
        finally:
            smoke.cleanup()
    print("Local Web smoke test passed; user data was isolated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
