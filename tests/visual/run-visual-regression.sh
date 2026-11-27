#!/bin/bash
# Compare every scenario that has both a real OpenIDEA and a real RustIdea capture.
# Missing captures are reported as "not run"; they are never replaced by mock art.
set -euo pipefail
here="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
root="$(CDPATH= cd -- "$here/../.." && pwd)"
manifest="$root/tests/visual-baselines/manifest.json"
out="$root/tests/visual-results"
mkdir -p "$out"

python3 - "$manifest" <<'PY' > /tmp/rustidea-visual-scenarios.txt
import json, sys
manifest = json.load(open(sys.argv[1]))
for scenario in manifest["scenarios"]:
    print(scenario["id"], scenario.get("openidea", ""), sep="\t")
PY

status=0
while IFS=$'\t' read -r id openidea; do
  rustidea="$root/tests/visual-baselines/rustidea/$id.png"
  openidea="$root/tests/visual-baselines/$openidea"
  if [[ -z "$openidea" || ! -f "$openidea" || ! -f "$rustidea" ]]; then
    echo "SKIP $id (missing real capture)"
    continue
  fi
  echo "== $id"
  python3 "$here/compare.py" "$openidea" "$rustidea" "$out/$id" \
    --masks "$(python3 - "$manifest" "$id" <<'PY'
import json, sys
manifest = json.load(open(sys.argv[1]))
scenario = next(s for s in manifest["scenarios"] if s["id"] == sys.argv[2])
print(json.dumps(scenario.get("masks", [])))
PY
)" --report-only
done < /tmp/rustidea-visual-scenarios.txt
exit $status
