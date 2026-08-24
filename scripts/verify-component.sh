#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
component=${1:-"$root/dist/provider-skylight-private.wasm"}
core=${2:-"$root/dist/provider-skylight-private.core.wasm"}
checksum="$component.sha256"
for file in "$component" "$core" "$checksum"; do
  test -f "$file" || { echo "error: missing generated artifact $file" >&2; exit 1; }
done

component_bytes=$(wc -c <"$component" | tr -d ' ')
if ((component_bytes > 393216)); then
  echo "error: component is $component_bytes bytes; maximum is 393216" >&2
  exit 1
fi
wasm-tools validate "$core"
wasm-tools validate "$component"

inspection=$(mktemp)
core_imports=$(mktemp)
trap 'rm -f "$inspection" "$core_imports"' EXIT
wasm-tools component wit --json "$component" >"$inspection"
wasm-tools print "$core" | grep '(import ' >"$core_imports" || true

python3 - "$root" "$component" "$core" "$checksum" "$inspection" <<'PY'
from pathlib import Path
import hashlib, json, re, sys
root, component, core, checksum, inspection = map(Path, sys.argv[1:])
expected_digest = hashlib.sha256(component.read_bytes()).hexdigest()
checksum_text = checksum.read_text()
expected_checksum = f"{expected_digest}  {component.name}\n"
if checksum_text != expected_checksum:
    raise SystemExit("error: component checksum is stale or contains a path")
model = json.loads(inspection.read_text())
if [package["name"] for package in model["packages"]] != [
    "dekopon:http@1.0.0", "root:component"
]:
    raise SystemExit(f"error: component packages are not exact: {model['packages']}")
if len(model["worlds"]) != 1 or model["worlds"][0]["name"] != "root":
    raise SystemExit("error: component must expose exactly one composed root world")
world = model["worlds"][0]
if world["imports"] != {"interface-0": {"interface": {"id": 0}}}:
    raise SystemExit(f"error: component import set is not exact: {world['imports']}")
if list(world["exports"]) != ["describe", "invoke"]:
    raise SystemExit(f"error: component export set/order is not exact: {list(world['exports'])}")
if len(model["interfaces"]) != 1:
    raise SystemExit("error: component must import exactly one interface")
interface = model["interfaces"][0]
if interface["name"] != "client" or list(interface["functions"]) != ["send"]:
    raise SystemExit("error: sole import is not dekopon:http/client.send")
rendered = json.dumps(model, sort_keys=True).lower()
for forbidden in (
    "wasi:", "filesystem", "environment", "clock", "random", "socket",
    "wasm-bindgen", "javascript", "js-sys",
):
    if forbidden in rendered:
        raise SystemExit(f"error: forbidden ambient interface/dependency in component: {forbidden}")
for artifact in (core, component):
    payload = artifact.read_bytes()
    for legal in (
        root / "THIRD_PARTY_NOTICES.md",
        root / "LICENSE-MIT",
        root / "LICENSE-APACHE",
        root / "security/wasm-dependencies.txt",
    ):
        if legal.read_bytes() not in payload:
            raise SystemExit(f"error: {artifact.name} does not embed exact {legal.name} bytes")
PY

if [[ ! -s "$core_imports" ]]; then
  echo 'error: core module did not retain its broker HTTP import' >&2
  exit 1
fi
if grep -Fv 'dekopon:http/client@1.0.0' "$core_imports"; then
  echo 'error: core module imports an interface other than Dekopon HTTP client 1.0.0' >&2
  exit 1
fi
core_metadata=$(wasm-tools metadata show "$core")
component_metadata=$(wasm-tools metadata show "$component")
grep -q 'Rust' <<<"$core_metadata"
grep -q 'wit-bindgen-rust' <<<"$component_metadata"

echo "component verified: imports=dekopon:http/client@1.0.0 exports=describe,invoke bytes=$component_bytes sha256=$(awk '{print $1}' "$checksum")"
