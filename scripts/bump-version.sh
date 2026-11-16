#!/usr/bin/env bash
# Prepare a reviewable 2.x version change. Never commit, tag or publish implicitly.
set -euo pipefail
PYTHON=python3
if [[ "${OS:-}" == "Windows_NT" ]]; then PYTHON=python; fi
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if [[ "${OS:-}" == "Windows_NT" ]]; then ROOT="$(cygpath -m "$ROOT")"; fi
"$PYTHON" - "$ROOT" "${1:-}" <<'PY'
import json, pathlib, re, sys
root=pathlib.Path(sys.argv[1]); version=sys.argv[2]
if not re.fullmatch(r'2\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?',version):
    raise SystemExit('Usage: bash scripts/bump-version.sh 2.x.y[-prerelease]')
if '-' in version and any(p.isdigit() and len(p)>1 and p.startswith('0') for p in version.split('-',1)[1].split('.')):
    raise SystemExit('Numeric prerelease identifiers cannot have leading zeros')
p=root/'Cargo.toml'; p.write_text(re.sub(r'(?m)^version = "[^"]+"',f'version = "{version}"',p.read_text(),count=1))
p=root/'Cargo.lock'; p.write_text(re.sub(r'(name = "(?:qinAegis-web|qin_aegis_core)"\nversion = ")[^"]+',lambda m:m[1]+version,p.read_text()))
for name in ['package.json','package-lock.json']:
    p=root/'crates/web_client/ui'/name; data=json.loads(p.read_text()); data['version']=version
    if name=='package-lock.json': data['packages']['']['version']=version
    p.write_text(json.dumps(data,indent=2)+'\n')
p=root/'scripts/install.sh';p.write_text(re.sub(r'(?m)^VERSION=\S+',f'VERSION={version}',p.read_text()))
PY
"$PYTHON" "$ROOT/scripts/release-assets.py" check
printf 'Prepared version changes. Review, test and commit before creating a release tag.\n'
