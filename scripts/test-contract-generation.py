#!/usr/bin/env python3
"""Real-service contract generation with a local deterministic model; no external AI."""
import importlib.util
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import sys
import tempfile
import threading
import time

loader = importlib.util.spec_from_file_location("local_web_smoke", Path(__file__).with_name("test-local-web.py"))
module = importlib.util.module_from_spec(loader)
loader.loader.exec_module(module)
require = module.require


def main():
    binary = Path(sys.argv[1]).resolve()
    arrived, release = threading.Event(), threading.Event()
    requests = []
    class Model(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass
        def do_POST(self):
            body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
            requests.append(body)
            call = len(requests)
            if call == 1:
                arrived.set()
                if not release.wait(10):
                    self.send_error(503)
                    return
            if call % 2:
                content = [{"id": f"receipt-{call}", "name": "Receipt", "requirement_id": "receipt",
                    "type": "smoke", "priority": "P1", "yaml_script": "tasks: []",
                    "expected_result": "receipt matches submitted input", "tags": []}]
            else:
                content = {"score": 90, "issues": [], "suggestions": [], "coverage": "receipt"}
            payload = json.dumps({"choices": [{"message": {"role": "assistant", "content": json.dumps(content)}}]}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
    model = ThreadingHTTPServer(("127.0.0.1", 0), Model)
    thread = threading.Thread(target=model.serve_forever, daemon=True)
    thread.start()
    with tempfile.TemporaryDirectory(prefix="qinaegis-generation-") as directory:
        smoke = module.Smoke(binary, directory)
        try:
            smoke.cli("start", "--no-open", "--port", str(smoke.port))
            smoke.instance = smoke.read_instance()
            smoke.authenticate()
            library = smoke.exercise_contracts()
            revision = 0
            for name in ("one", "two"):
                profile = {"id": name, "label": name, "adapter": "openai_compatible",
                    "base_url": f"http://127.0.0.1:{model.server_port}/v1", "model": f"model-{name}",
                    "api_key": "fake-local-key", "capabilities": ["text"], "enabled": True,
                    "allow_remote": False, "midscene_family": None}
                smoke.secrets.append(profile["api_key"])
                state = smoke.rpc("saveModel", {"profile": profile, "expected_revision": revision})
                revision = state["revision"]
            state = smoke.rpc("setModelRoute", {"task": "generate", "route": {"primary": "one", "fallbacks": []}, "expected_revision": revision})
            preview = smoke.rpc("getContractExecution", {"project": "smoke-alpha"})
            require(preview["contracts"][0]["publication"]["definition"]["version"] == 1,
                    "preview silently upgraded an old binding")
            job = smoke.rpc("runGenerate", {"project": "smoke-alpha", "requirement": "receipt", "use_contracts": True})
            require(arrived.wait(5), "local model did not receive generation")
            status = smoke.rpc("getJobStatus", {"jobId": job["jobId"]})
            require(status["snapshot"]["model"]["candidates"][0]["id"] == "one"
                    and status["snapshot"]["contracts"] == preview, "generation did not pin contracts and model")
            smoke.rpc("setModelRoute", {"task": "generate", "route": {"primary": "two", "fallbacks": []}, "expected_revision": state["revision"]})
            smoke.rpc("bindContract", {"project": "smoke-alpha", "contract_id": "receipt", "version": 2,
                "parameters": {"target": "https://alpha-new.invalid", "quantity": 3}, "expected_revision": library["revision"]})
            release.set()
            def finish(job_id):
                deadline = time.monotonic() + 10
                while time.monotonic() < deadline:
                    result = smoke.rpc("getJobStatus", {"jobId": job_id})
                    if result["status"] in ("completed", "failed", "stopped"):
                        require(result["status"] == "completed", "deterministic generation did not complete")
                        return result
                    time.sleep(0.02)
                raise module.SmokeFailure("generation timed out")
            first = finish(job["jobId"])
            require(first["snapshot"] == status["snapshot"], "running snapshot changed after profile or binding edit")
            require(len(requests) == 2 and all(r["model"] == "model-one" for r in requests), "running generator/critic changed models")
            prompt = json.dumps(requests[0]["messages"])
            require("alpha-project-input.invalid" in prompt and "alpha-new.invalid" not in prompt
                    and "receipt has a timestamp" not in prompt, "running contract inputs or assertions drifted")
            second_job = smoke.rpc("runGenerate", {"project": "smoke-beta", "requirement": "receipt", "use_contracts": True})
            second = finish(second_job["jobId"])
            require(second["snapshot"]["model"]["candidates"][0]["id"] == "two"
                    and all(r["model"] == "model-two" for r in requests[2:]), "next job did not use switched model")
            require("beta-project-input.invalid" in json.dumps(requests[2]["messages"]), "second project reused first project inputs")
            projects = smoke.data / "projects"
            for project in ("smoke-alpha", "smoke-beta"):
                saved = list((projects / project / "cases" / "draft").glob("*.json"))
                require(len(saved) == 1, "generated draft was not saved to selected project")
                require(json.loads(saved[0].read_text())["status"] == "draft", "model bypassed human case review")
            require(not (projects / "default").exists(), "generation wrote to hardcoded default project")
            print("PASS two-project generation, immutable contract/model snapshots, next-job switching and draft review boundary")
        finally:
            release.set()
            # A failed assertion may release an in-flight request. Wait for the
            # real worker to exit before requesting shutdown of our instance.
            for _ in range(100):
                try:
                    smoke.cli("stop")
                    break
                except module.SmokeFailure:
                    time.sleep(0.05)
            smoke.cleanup()
            model.shutdown()
            model.server_close()


if __name__ == "__main__":
    main()
