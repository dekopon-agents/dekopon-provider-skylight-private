#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
cd "$root"
git diff --exit-code
git diff --cached --exit-code
test -n "$(git rev-parse HEAD)"

temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT
first="$temporary/first"
second="$temporary/second"
git clone --quiet --no-local --no-hardlinks "$root" "$first"
git clone --quiet --no-local --no-hardlinks "$root" "$second"
for checkout in "$first" "$second"; do
  git -C "$checkout" checkout --quiet --detach HEAD
  (
    cd "$checkout"
    ./build.sh
    scripts/generate-sbom.sh
    scripts/verify-component.sh
    test -z "$(git status --porcelain --untracked-files=all)"
  )
done

outputs=(
  provider-skylight-private.core.wasm
  provider-skylight-private.wasm
  provider-skylight-private.wasm.sha256
  provider-skylight-private.cdx.json
  provider-skylight-private.dependency-inventory.json
)
for output in "${outputs[@]}"; do
  cmp "$first/dist/$output" "$second/dist/$output"
done
for legal in THIRD_PARTY_NOTICES.md LICENSE-MIT LICENSE-APACHE security/wasm-dependencies.txt; do
  cmp "$first/$legal" "$second/$legal"
  python3 - "$first/$legal" "$first/dist/provider-skylight-private.core.wasm" \
    "$first/dist/provider-skylight-private.wasm" <<'PY'
from pathlib import Path
import sys
needle = Path(sys.argv[1]).read_bytes()
for artifact in map(Path, sys.argv[2:]):
    if needle not in artifact.read_bytes():
        raise SystemExit(f"error: {artifact} does not contain exact legal bytes from {sys.argv[1]}")
PY
done

digest=$(awk '{print $1}' "$first/dist/provider-skylight-private.wasm.sha256")
printf 'two independent clean checkouts reproduced core, component, checksum, SBOM, inventory, and legal bytes: %s\n' "$digest"
