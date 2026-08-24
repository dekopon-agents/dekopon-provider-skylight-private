#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
cd "$root"
test -f Cargo.lock || { echo 'error: Cargo.lock is required' >&2; exit 1; }

python3 - "$root" <<'PY'
from pathlib import Path
import sys, tomllib
root = Path(sys.argv[1])
manifest = tomllib.loads((root / "Cargo.toml").read_text())
for table_name in ("dependencies", "dev-dependencies", "build-dependencies"):
    for name, value in manifest.get(table_name, {}).items():
        if isinstance(value, str):
            version = value
            table = {}
        else:
            table = value
            version = value.get("version", "")
        if "path" in table or "git" in table:
            raise SystemExit(f"error: {table_name}.{name} is a local or Git dependency")
        if not version.startswith("="):
            raise SystemExit(f"error: {table_name}.{name} is not an exact crates.io pin: {version!r}")
        if name.startswith("dekopon-") and version != "=0.11.1":
            raise SystemExit(f"error: {table_name}.{name} must be exactly =0.11.1")
lock = tomllib.loads((root / "Cargo.lock").read_text())
for package in lock["package"]:
    source = package.get("source")
    if source is not None and source != "registry+https://github.com/rust-lang/crates.io-index":
        raise SystemExit(
            f"error: lock entry {package['name']} {package['version']} has forbidden source {source}"
        )
PY

metadata=$(mktemp)
tree=$(mktemp)
features=$(mktemp)
trap 'rm -f "$metadata" "$tree" "$features"' EXIT
cargo metadata --locked --format-version 1 --filter-platform wasm32-unknown-unknown >"$metadata"
cargo tree --locked --target wasm32-unknown-unknown --edges normal,build \
  --prefix none --format '{p}' >"$tree"
cargo tree --locked --target wasm32-unknown-unknown --edges normal,build,features >"$features"

python3 - "$root" "$metadata" <<'PY'
from pathlib import Path
import json, sys
root = Path(sys.argv[1])
metadata = json.loads(Path(sys.argv[2]).read_text())
packages = metadata["packages"]
expected = {
    ("dekopon-provider-sdk", "0.11.1"): "wit/provider.wit",
    ("dekopon-provider-http", "0.11.1"): "wit/deps/http.wit",
}
resolved = {}
for package in packages:
    key = (package["name"], package["version"])
    if key in expected and package["source"] == "registry+https://github.com/rust-lang/crates.io-index":
        if key in resolved:
            raise SystemExit(f"error: duplicate resolved crates.io package {key}")
        resolved[key] = Path(package["manifest_path"]).parent / expected[key]
if set(resolved) != set(expected):
    raise SystemExit(f"error: missing resolved WIT owner(s): {set(expected) - set(resolved)}")
comparisons = [
    (root / "wit/deps/provider.wit", resolved[("dekopon-provider-sdk", "0.11.1")]),
    (root / "wit/deps/http.wit", resolved[("dekopon-provider-http", "0.11.1")]),
]
for mirror, owner in comparisons:
    if mirror.read_bytes() != owner.read_bytes():
        raise SystemExit(f"error: {mirror.relative_to(root)} differs from locked crates.io owner {owner}")
PY

if grep -E '(^|[/ ])(wasi|wasi-core|wasi-ext|wasm-bindgen|js-sys)( |v|$)' "$tree"; then
  echo 'error: shipped Wasm closure contains a forbidden ambient/JavaScript dependency' >&2
  exit 1
fi
for package in dekopon-provider-http dekopon-provider-sdk; do
  grep -Eq "^${package} v0\\.11\\.1([[:space:]]|$)" "$tree" || {
    echo "error: shipped graph does not contain crates.io $package 0.11.1" >&2
    exit 1
  }
done
grep -Fq 'wit-bindgen feature "macros"' "$features"
grep -Fq 'wit-bindgen feature "realloc"' "$features"
python3 scripts/dependency_inventory.py --check

echo 'locked crates.io sources, exact pins, shipped feature graph, and owner WIT mirrors verified'
