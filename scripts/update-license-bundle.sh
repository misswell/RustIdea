#!/bin/bash
set -euo pipefail

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
report="/tmp/rustidea-about.json"
output="$repo_root/THIRD_PARTY_RUST_LICENSES.md"

cd "$repo_root"
cargo about generate \
  --manifest-path crates/rustidea/Cargo.toml \
  --config about.toml \
  --format json \
  --target aarch64-apple-darwin \
  --locked \
  --offline \
  --fail \
  --output-file "$report"

python3 - "$report" "$output" <<'PY'
import datetime
import json
import sys

report_path, output_path = sys.argv[1:3]
with open(report_path, encoding="utf-8") as source:
    report = json.load(source)

crates = report["crates"]
licenses = report["licenses"]
license_ids = sorted({entry["id"] for entry in licenses})

lines = [
    "# Rust third-party license notices",
    "",
    "Generated from `Cargo.lock` for `rustidea` and target `aarch64-apple-darwin` "
    f"using `cargo-about 0.9.3` with `--locked --offline --fail` on {datetime.date.today().isoformat()}.",
    "",
    f"The dependency graph contains {len(crates)} crate records and "
    f"{len(license_ids)} selected SPDX license identifiers. The list includes build "
    "dependencies resolved for this target. It is a generated inventory, not a legal "
    "opinion or proof of the exact code linked into the final executable.",
    "",
    "## Crate inventory",
    "",
]

for crate in sorted(
    crates, key=lambda entry: (entry["package"]["name"].lower(), entry["package"]["version"])
):
    package = crate["package"]
    lines.append(
        f"- `{package['name']} {package['version']}` — `{crate['license']}`"
    )

lines += ["", "## License texts", ""]
seen_texts = set()
for entry in sorted(licenses, key=lambda entry: (entry["id"], entry["text"])):
    key = (entry["id"], entry["text"])
    if key in seen_texts:
        continue
    seen_texts.add(key)
    lines += [
        f"### {entry['id']}",
        "",
        f"Source: {entry.get('source_path') or entry.get('name', 'unknown')}",
        "",
        "```text",
        entry["text"].rstrip(),
        "```",
        "",
    ]

with open(output_path, "w", encoding="utf-8") as target:
    target.write("\n".join(lines))
PY

wc -c "$output"
