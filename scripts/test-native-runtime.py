#!/usr/bin/env python3
"""Isolated native HTTP runtime acceptance; standard library only.

Small archive fixtures verify transport/preview/recovery, not a working engine.
--archive optionally verifies and ACTIVATES a real host runtime in isolated data.
No model, browser download, publisher network or user configuration is used.
"""
import argparse
import hashlib
import http.client
import importlib.util
import io
import json
import os
from pathlib import Path
import platform
import sys
import tarfile
import tempfile
import time

spec = importlib.util.spec_from_file_location("local_web_smoke", Path(__file__).with_name("test-local-web.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
Smoke, require, SmokeFailure = module.Smoke, module.require, module.SmokeFailure
CHUNK = 512 * 1024


def fixture():
    """Digest-correct but deliberately NON-EXECUTABLE engine fixture."""
    digest = lambda b: hashlib.sha256(b).hexdigest()
    system = {"Darwin": "darwin", "Windows": "windows", "Linux": "linux"}[platform.system()]
    arch = platform.machine().lower()
    arch = {"aarch64": "arm64", "amd64": "amd64"}.get(arch, arch)
    if system == "linux" and arch == "arm64":
        arch = "aarch64"
    files = {"entry.js": b"fixture only", "node": b"non executable fixture", "pnpm-lock.yaml": b"fixture lock"}
    files["runtime.json"] = json.dumps({"schema_version": 1, "engine": "midscene", "version": "2.0.0-fixture", "entry": "entry.js", "node": "node"}).encode()
    files["provenance.json"] = json.dumps({"schema_version": 1, "version": "2.0.0-fixture", "target": f"{system}-{arch}", "source_commit": "a" * 40, "node_version": "v24.1", "compiled_sha256": {"entry.js": digest(files["entry.js"])}, "node_sha256": digest(files["node"]), "lock_sha256": digest(files["pnpm-lock.yaml"])}).encode()
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w:gz") as archive:
        for name, data in files.items():
            info = tarfile.TarInfo(name)
            info.size, info.mode = len(data), 0o700
            archive.addfile(info, io.BytesIO(data))
    return buffer.getvalue()


def make_upgrade_fixture(source, destination):
    """Keep genuine executable content; label changed metadata as an acceptance fixture.

    Stream members without extracting paths or loading the installed tree into RAM.
    This locally derived archive is never a publisher/stable-release artifact.
    """
    version = "2.0.0-native-upgrade-fixture"
    changed = set()
    with tarfile.open(source, "r|gz") as incoming, tarfile.open(destination, "w|gz", format=tarfile.PAX_FORMAT, compresslevel=1) as outgoing:
        for member in incoming:
            reader = incoming.extractfile(member) if member.isfile() else None
            if member.name.lstrip("./") in ("runtime.json", "provenance.json"):
                require(reader is not None and member.size <= 1024 * 1024, "invalid fixture metadata")
                value = json.loads(reader.read())
                value["version"] = version
                if member.name.lstrip("./") == "provenance.json":
                    value["acceptance_fixture"] = "Locally derived for native upgrade/rollback testing; not an official release"
                content = json.dumps(value).encode()
                member.size = len(content)
                reader = io.BytesIO(content)
                changed.add(member.name.lstrip("./"))
            outgoing.addfile(member, reader)
            if reader is not None:
                reader.close()
    require(changed == {"runtime.json", "provenance.json"}, "archive lacks fixture metadata")
    return version


class NativeSmoke(Smoke):
    def chunk(self, upload, offset, data, expected=200, headers=None):
        conn = http.client.HTTPConnection("127.0.0.1", self.port, timeout=15)
        try:
            conn.request("PUT", f"/api/v2/runtime-upload/{upload}/{offset}", data,
                         self.authorized if headers is None else headers)
            response = conn.getresponse()
            body = response.read()
            require(response.status == expected, f"upload expected {expected}, got {response.status}")
            return json.loads(body)
        finally:
            conn.close()

    def wait(self, job, status="completed", timeout=60):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            state = self.rpc("getJobStatus", {"jobId": job["jobId"]})
            if state.get("status") in ("completed", "failed", "stopped"):
                require(state["status"] == status, f"job expected {status}, got {state['status']}: {state.get('error')}")
                return state.get("result")
            time.sleep(0.05)
        raise SmokeFailure("runtime job exceeded deadline")

    def restart(self):
        self.cli("stop")
        self.instance = None
        self.cli("start", "--no-open", "--port", str(self.port))
        self.instance = self.read_instance()
        self.authenticate()

    def begin(self, data, sha=None):
        return self.rpc("beginRuntimeUpload", {"size": len(data), "expected_sha256": sha or hashlib.sha256(data).hexdigest()})["upload_id"]

    def upload(self, data):
        upload = self.begin(data)
        for offset in range(0, len(data), CHUNK):
            result = self.chunk(upload, offset, data[offset:offset+CHUNK])
            require(result["received"] == min(len(data), offset+CHUNK), "upload offset mismatch")
        return upload

    def confirm(self, preview, discard=False, status="completed"):
        method = "confirmRuntimeRollback" if preview["operation"] == "rollback" else "confirmRuntimeInstall"
        return self.wait(self.rpc(method, {"preview_token": preview["preview_token"],
            "expected_revision": preview["expected_revision"], "discard_previous": discard,
            "approved": True}), status=status, timeout=600)

    def exercise_switches(self, archive, original):
        with tempfile.TemporaryDirectory(prefix="native-upgrade-fixture-", dir=self.home) as directory:
            variant = Path(directory) / "upgrade-fixture.tar.gz"
            version = make_upgrade_fixture(archive, variant)
            variant_data = variant.read_bytes()
            upgraded_sha = hashlib.sha256(variant_data).hexdigest()
            preview = self.wait(self.rpc("prepareRuntimeInstall", {"upload_id": self.upload(variant_data)}), timeout=600)
            require(preview["candidate"]["version"] == version, "upgrade preview version mismatch")
            upgraded = self.confirm(preview)
            require(upgraded["current"]["archive_sha256"] == upgraded_sha and upgraded["previous"] == original["current"], "upgrade did not preserve original")
            print("PASS real-engine derived-fixture upgrade retained original runtime", flush=True)
            self.restart()
            persisted = self.rpc("getRuntimeMaintenance")
            require(persisted["current"] == upgraded["current"] and persisted["previous"] == upgraded["previous"], "upgrade state failed restart persistence")
            rollback = self.wait(self.rpc("prepareRuntimeRollback"), timeout=600)
            restored = self.confirm(rollback)
            require(restored["current"] == original["current"] and restored["previous"] == upgraded["current"], "rollback did not swap verified versions")
            print("PASS real-engine rollback swapped verified current and previous", flush=True)
            preview = self.wait(self.rpc("prepareRuntimeInstall", {"upload_id": self.upload(variant_data)}), timeout=600)
            require(preview["discard_previous_required"], "older backup approval was not required")
            self.confirm(preview, status="failed")
            refused = self.rpc("getRuntimeMaintenance")
            require(refused["current"] == restored["current"] and refused["previous"] == restored["previous"], "refused discard changed installed versions")
            switched = self.confirm(preview, discard=True)
            require(switched["current"]["archive_sha256"] == upgraded_sha and switched["previous"] == original["current"], "approved older backup replacement failed")
            preview = self.wait(self.rpc("prepareRuntimeInstall", {"upload_id": self.upload(fixture())}), timeout=600)
            self.confirm(preview, discard=True, status="failed")
            failed = self.rpc("getRuntimeMaintenance")
            require(failed["current"] == switched["current"] and failed["previous"] == switched["previous"], "failed candidate health damaged installed versions")
            self.wait(self.rpc("discardRuntimePreview", {"preview_token": preview["preview_token"]}), timeout=600)
            self.restart()
            final = self.rpc("getRuntimeMaintenance")
            require(final["current"] == switched["current"] and final["previous"] == switched["previous"], "final installed state failed restart persistence")
            print("PASS genuine bundled Node/import health through derived-fixture upgrade, rollback, explicit older-backup discard, failed health preservation and restart (no browser/model)", flush=True)

    def run_native(self, archive, exercise_switches=False):
        self.cli("start", "--no-open", "--port", str(self.port))
        self.instance = self.read_instance()
        self.authenticate()
        require(self.rpc("getRuntimeMaintenance")["current"] is None, "fresh runtime unexpectedly installed")
        data = b"upload boundary fixture"
        upload = self.begin(data)
        self.chunk(upload, 0, data, expected=401, headers={"Origin": self.origin})
        self.chunk(upload, 0, data, expected=403, headers={**self.authorized, "Origin": "https://evil.invalid"})
        self.chunk(upload, 0, data, expected=401, headers={**self.authorized, "x-qinaegis-csrf": "wrong"})
        self.chunk(upload, 0, b"x" * (CHUNK+1), expected=413)
        self.chunk(upload, 1, data, expected=409)
        self.chunk(upload, 0, data[:4])
        self.wait(self.rpc("prepareRuntimeInstall", {"upload_id": upload}), "failed")
        self.restart()
        require(self.rpc("getRuntimeMaintenance")["upload"]["received"] == 4, "restart lost transfer offset")
        self.chunk(upload, 4, data[4:])
        self.chunk(upload, len(data), b"overflow", expected=409)
        self.wait(self.rpc("prepareRuntimeInstall", {"upload_id": upload}), "failed")
        require(self.rpc("getRuntimeMaintenance")["upload"]["claimed"] is False, "failure retained claim")
        self.rpc("abortRuntimeUpload", {"upload_id": upload})
        require(self.rpc("getRuntimeMaintenance")["upload"] is None, "discard retained transfer")
        print("PASS native upload session/Origin/CSRF, chunk/offset/size bounds, incomplete archive, restart and failed claim release", flush=True)
        data = fixture()
        upload = self.begin(data, "0" * 64)
        self.chunk(upload, 0, data)
        self.wait(self.rpc("prepareRuntimeInstall", {"upload_id": upload}), "failed")
        require(self.rpc("getRuntimeMaintenance")["preview"] is None, "bad digest created preview")
        self.rpc("abortRuntimeUpload", {"upload_id": upload})
        preview = self.wait(self.rpc("prepareRuntimeInstall", {"upload_id": self.upload(data)}))
        require(preview["operation"] == "install", "wrong preview operation")
        self.restart()
        restored = self.rpc("getRuntimeMaintenance")["preview"]
        require(restored == preview, "preview failed restart persistence")
        confirmation = {"preview_token": preview["preview_token"], "expected_revision": preview["expected_revision"], "discard_previous": False, "approved": False}
        self.rpc("confirmRuntimeInstall", confirmation, success=False)
        self.wait(self.rpc("confirmRuntimeInstall", {**confirmation, "approved": True, "expected_revision": "999999"}), "failed")
        # Deliberately invalid executable must fail health and preserve the empty current slot.
        self.wait(self.rpc("confirmRuntimeInstall", {**confirmation, "approved": True}), "failed")
        require(self.rpc("getRuntimeMaintenance")["current"] is None, "failed health activated fixture")
        self.wait(self.rpc("discardRuntimePreview", {"preview_token": preview["preview_token"]}))
        require(self.rpc("getRuntimeMaintenance")["preview"] is None, "preview discard failed")
        self.wait(self.rpc("prepareRuntimeRollback"), "failed")
        print("PASS digest refusal, preview persistence, explicit approval, revision guard, health refusal and preview discard (fixture only)", flush=True)
        if archive:
            data = archive.read_bytes()
            preview = self.wait(self.rpc("prepareRuntimeInstall", {"upload_id": self.upload(data)}), timeout=600)
            result = self.wait(self.rpc("confirmRuntimeInstall", {"preview_token": preview["preview_token"], "expected_revision": preview["expected_revision"], "discard_previous": False, "approved": True}), timeout=600)
            require(result["current"]["archive_sha256"] == hashlib.sha256(data).hexdigest(), "real runtime digest mismatch")
            print("PASS real host archive native HTTP install and bundled Node/import health (no browser/model)", flush=True)
            if exercise_switches:
                self.exercise_switches(archive, result)
        self.cli("stop")
        self.instance = None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--archive", type=Path)
    parser.add_argument("--exercise-switches", action="store_true", help="derive a labeled real-engine fixture to verify upgrade/rollback; requires --archive")
    args = parser.parse_args()
    if args.exercise_switches and not args.archive:
        parser.error("--exercise-switches requires --archive")
    if not args.binary.is_absolute() or not args.binary.is_file():
        parser.error("binary must be an absolute executable path")
    with tempfile.TemporaryDirectory(prefix="qinaegis-native-http-") as directory:
        smoke = NativeSmoke(args.binary.resolve(), Path(directory).resolve())
        try:
            smoke.run_native(args.archive.resolve() if args.archive else None, args.exercise_switches)
        except (SmokeFailure, OSError, ValueError) as error:
            print(f"FAIL {error}", file=sys.stderr)
            return 1
        finally:
            smoke.cleanup()
    print("Native runtime HTTP acceptance passed; user data was isolated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
