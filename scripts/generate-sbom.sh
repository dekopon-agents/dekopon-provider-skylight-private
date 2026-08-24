#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
dist="$root/dist"
component="$dist/provider-skylight-private.wasm"
inventory="$dist/provider-skylight-private.dependency-inventory.json"
sbom="$dist/provider-skylight-private.cdx.json"
test -f "$component" || { echo "error: build the component before generating its SBOM" >&2; exit 1; }
mkdir -p "$dist"

python3 "$root/scripts/dependency_inventory.py" --format json --output "$inventory"
python3 "$root/scripts/dependency_inventory.py" --format cyclonedx --output "$sbom"
python3 - "$component" "$sbom" <<'PY'
from pathlib import Path
import hashlib, json, sys
component = Path(sys.argv[1])
sbom = Path(sys.argv[2])
payload = component.read_bytes()
document = json.loads(sbom.read_text())
metadata_component = document["metadata"]["component"]
metadata_component["hashes"] = [
    {"alg": "SHA-256", "content": hashlib.sha256(payload).hexdigest()}
]
document["metadata"]["properties"].extend(
    [
        {"name": "dekopon:component-bytes", "value": str(len(payload))},
        {"name": "dekopon:rust-toolchain", "value": "1.89.0"},
        {"name": "dekopon:wasm-tools", "value": "1.236.1"},
    ]
)
sbom.write_text(
    json.dumps(document, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
    encoding="utf-8",
    newline="\n",
)
PY

# Render a second time in an isolated temporary directory and require byte identity.
temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT
python3 "$root/scripts/dependency_inventory.py" --format json --output "$temporary/inventory.json"
python3 "$root/scripts/dependency_inventory.py" --format cyclonedx --output "$temporary/sbom.json"
python3 - "$component" "$temporary/sbom.json" <<'PY'
from pathlib import Path
import hashlib, json, sys
component = Path(sys.argv[1])
sbom = Path(sys.argv[2])
payload = component.read_bytes()
document = json.loads(sbom.read_text())
document["metadata"]["component"]["hashes"] = [
    {"alg": "SHA-256", "content": hashlib.sha256(payload).hexdigest()}
]
document["metadata"]["properties"].extend(
    [
        {"name": "dekopon:component-bytes", "value": str(len(payload))},
        {"name": "dekopon:rust-toolchain", "value": "1.89.0"},
        {"name": "dekopon:wasm-tools", "value": "1.236.1"},
    ]
)
sbom.write_text(json.dumps(document, indent=2, sort_keys=True, ensure_ascii=False) + "\n", newline="\n")
PY
cmp "$inventory" "$temporary/inventory.json"
cmp "$sbom" "$temporary/sbom.json"
printf 'generated deterministic inventory %s and CycloneDX SBOM %s\n' "$inventory" "$sbom"
