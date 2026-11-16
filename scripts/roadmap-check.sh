#!/usr/bin/env bash
# Run reproducible 2.0 checks. Passing checks do not imply product acceptance.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"
python3 - <<'PY'
from datetime import datetime, timezone
from pathlib import Path
import subprocess
import sys

root = Path.cwd()
logs = root / 'build/roadmap-check'
logs.mkdir(parents=True, exist_ok=True)
checks = [
    ('Source versions', ['python3', 'scripts/release-assets.py', 'check'], root),
    ('Release integrity regressions', ['python3', 'scripts/test-release-assets.py'], root),
    ('Installer regressions', ['python3', 'scripts/test-install.py'], root),
    ('UI regressions', ['npm', 'test'], root / 'crates/web_client/ui'),
    ('Embedded Web UI build', ['npm', 'run', 'build'], root / 'crates/web_client/ui'),
    ('Public Pages build', ['npm', 'run', 'build:pages'], root / 'crates/web_client/ui'),
    ('Rust workspace regressions', ['cargo', 'test', '--workspace', '--locked'], root),
]
print('# QinAegis 2.0 verification status\n')
print(f'Generated: {datetime.now(timezone.utc).isoformat()}\n')
print('These checks verify the repository. They do not certify 2.0 release readiness.\n')
print('| Check | Result | Exit code | Log |\n|---|---|---|---|', flush=True)
failed = []
for index, (label, command, cwd) in enumerate(checks, 1):
    log = logs / f'{index}.log'
    try:
        with log.open('w') as output:
            result = subprocess.run(command, cwd=cwd, stdout=output, stderr=subprocess.STDOUT, timeout=1200)
            code = result.returncode
    except (OSError, subprocess.TimeoutExpired) as error:
        with log.open('a') as output:
            output.write(f'\n{error}\n')
        code = 124 if isinstance(error, subprocess.TimeoutExpired) else 127
    print(f'| {label} | {"PASS" if code == 0 else "FAIL"} | {code} | {log.relative_to(root)} |', flush=True)
    if code:
        failed.append(label)
        print(f'\nFailed: {label}; see {log.relative_to(root)}', file=sys.stderr)
        print('\n'.join(log.read_text(errors='replace').splitlines()[-25:]), file=sys.stderr)
print('\n## Product acceptance still required\n')
print('- Real business visual execution using Playwright + Midscene.js across two projects sharing a contract.')
print('- Clean-machine installation, runtime preparation and browser launch on supported operating systems.')
print('- Remaining runtime installation, orchestration, recovery and accessibility milestones in docs/progress/local-web-2.0.md.')
print('\nScheduled checks and candidate archives are not a published stable release or an updated production Pages site.')
if failed:
    print('\nFailed checks: ' + ', '.join(failed))
sys.exit(bool(failed))
PY
