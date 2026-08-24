#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
component="$root/dist/provider-skylight-private.wasm"
test -f "$component" || { echo 'error: run ./build.sh first' >&2; exit 1; }
DEKOPON_SKYLIGHT_COMPONENT="$component" \
  cargo test --locked --manifest-path "$root/Cargo.toml" \
  --test component_host immediate_host_refuses_the_sole_privileged_import -- --exact

echo 'immediate host refusal verified without dispatch or network access'
