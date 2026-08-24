#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
manifest="$root/Cargo.toml"
target_dir=${CARGO_TARGET_DIR:-"$root/target"}
core_build="$target_dir/wasm32-unknown-unknown/release/dekopon_skylight_private_provider.wasm"
dist="$root/dist"
core="$dist/provider-skylight-private.core.wasm"
component="$dist/provider-skylight-private.wasm"
checksum="$component.sha256"

rust_toolchain="1.89.0"
required_rustc="rustc 1.89.0 (29483883e 2025-08-04)"
required_wasm_tools_version="1.236.1"

command -v rustup >/dev/null 2>&1 || {
  echo "error: rustup with Rust $rust_toolchain is required" >&2
  exit 1
}
actual_rustc=$(rustup run "$rust_toolchain" rustc --version 2>/dev/null) || {
  echo "error: Rust $rust_toolchain is required" >&2
  exit 1
}
if [[ "$actual_rustc" != "$required_rustc" ]]; then
  echo "error: expected $required_rustc, found $actual_rustc" >&2
  exit 1
fi
command -v wasm-tools >/dev/null 2>&1 || {
  echo "error: wasm-tools $required_wasm_tools_version is required" >&2
  exit 1
}
actual_wasm_tools=$(wasm-tools --version)
actual_wasm_tools_version=${actual_wasm_tools#wasm-tools }
actual_wasm_tools_version=${actual_wasm_tools_version%% *}
if [[ "$actual_wasm_tools_version" != "$required_wasm_tools_version" ]]; then
  echo "error: expected wasm-tools $required_wasm_tools_version, found $actual_wasm_tools" >&2
  exit 1
fi

python3 "$root/scripts/dependency_inventory.py" --check

cargo_home=${CARGO_HOME:-"$HOME/.cargo"}
cargo_home=$(cd "$cargo_home" && pwd -P)
sysroot=$(rustup run "$rust_toolchain" rustc --print sysroot)
sysroot=$(cd "$sysroot" && pwd -P)
rustc_path=$(rustup which --toolchain "$rust_toolchain" rustc)
rustc_proxy="$target_dir/deterministic-rustc"
mkdir -p "$(dirname "$rustc_proxy")" "$dist"
cat >"$rustc_proxy" <<'PROXY'
#!/usr/bin/env bash
set -euo pipefail

actual_rustc=${DEKOPON_BUILD_RUSTC:?}
source_root=${DEKOPON_BUILD_SOURCE_ROOT:?}
manifest_dir=${CARGO_MANIFEST_DIR-}
repository_crate=false
if [[ "$manifest_dir" == "$source_root" || "$manifest_dir" == "$source_root/"* ]]; then
  repository_crate=true
fi

target=host
expect_target=false
for argument in "$@"; do
  if [[ "$expect_target" == true ]]; then
    target=$argument
    expect_target=false
    continue
  fi
  case $argument in
    --target) expect_target=true ;;
    --target=*) target=${argument#--target=} ;;
  esac
done

normalize_metadata=$repository_crate
if [[ "$target" == wasm32-unknown-unknown ]]; then
  normalize_metadata=true
fi

args=()
crate_name=
while (($#)); do
  case $1 in
    --crate-name)
      crate_name=$2
      args+=("$1" "$2")
      shift 2
      ;;
    --target)
      target=$2
      args+=("$1" "$2")
      shift 2
      ;;
    --target=*)
      target=${1#--target=}
      args+=("$1")
      shift
      ;;
    -C)
      if (($# >= 2)) && [[ $2 == metadata=* ]] && [[ "$normalize_metadata" == true ]]; then
        shift 2
      else
        args+=("$1")
        shift
      fi
      ;;
    -Cmetadata=*)
      if [[ "$normalize_metadata" == true ]]; then
        shift
      else
        args+=("$1")
        shift
      fi
      ;;
    *)
      args+=("$1")
      shift
      ;;
  esac
done

if [[ "$normalize_metadata" == true && -n "$crate_name" && -n "${CARGO_PKG_NAME-}" && -n "${CARGO_PKG_VERSION-}" ]]; then
  args+=(
    -C
    "metadata=dekopon-skylight-standalone-repro-v1-${CARGO_PKG_NAME}-${CARGO_PKG_VERSION}-$crate_name-$target"
  )
fi
exec "$actual_rustc" "${args[@]}"
PROXY
chmod 0700 "$rustc_proxy"

# Cargo's checkout-dependent package IDs and target dependency hashes enter rustc metadata. The
# proxy remains behind any configured RUSTC_WRAPPER (including sccache), delegates to the pinned
# compiler, and normalizes metadata for the repository crate and every Wasm-target crate.
printf -v encoded_rustflags '%s\x1f%s\x1f%s\x1f%s\x1f%s\x1f%s' \
  "--remap-path-prefix=$root=/dekopon-provider-skylight-private/source" \
  "--remap-path-prefix=$cargo_home=/dekopon/cargo" \
  "--remap-path-prefix=$sysroot=/dekopon/rust/$rust_toolchain" \
  '--cfg=dekopon_skylight_standalone_repro_v1' \
  '--check-cfg=cfg(dekopon_skylight_standalone_repro_v1)' \
  '-Ccodegen-units=1'

rustup target add --toolchain "$rust_toolchain" wasm32-unknown-unknown >/dev/null
CARGO_ENCODED_RUSTFLAGS="$encoded_rustflags" \
  DEKOPON_BUILD_RUSTC="$rustc_path" \
  DEKOPON_BUILD_SOURCE_ROOT="$root" \
  RUSTC="$rustc_proxy" \
  rustup run "$rust_toolchain" cargo build \
  --locked --manifest-path "$manifest" --target wasm32-unknown-unknown --release

install -m 0644 "$core_build" "$core"
wasm-tools component new "$core" -o "$component"

python3 - "$root" "$cargo_home" "$sysroot" "$target_dir" "$core" "$component" <<'PY'
from pathlib import Path
import sys
root = Path(sys.argv[1])
local_paths = [Path(path) for path in sys.argv[1:5]]
artifacts = [Path(path) for path in sys.argv[5:]]
legal = [
    root / "THIRD_PARTY_NOTICES.md",
    root / "LICENSE-MIT",
    root / "LICENSE-APACHE",
    root / "security/wasm-dependencies.txt",
]
for artifact in artifacts:
    payload = artifact.read_bytes()
    for source in legal:
        if source.read_bytes() not in payload:
            raise SystemExit(f"error: {artifact} does not embed exact bytes from {source}")
    for forbidden in local_paths:
        if str(forbidden).encode() in payload:
            raise SystemExit(f"error: {artifact} embeds local build path {forbidden}")
PY

component_bytes=$(wc -c <"$component" | tr -d ' ')
if ((component_bytes > 393216)); then
  echo "error: component is $component_bytes bytes; maximum is 393216" >&2
  exit 1
fi

if command -v sha256sum >/dev/null 2>&1; then
  digest=$(sha256sum "$component" | awk '{print $1}')
else
  digest=$(shasum -a 256 "$component" | awk '{print $1}')
fi
printf '%s  %s\n' "$digest" "$(basename "$component")" >"$checksum"
printf 'generated %s (%s bytes, sha256 %s) with Rust %s and wasm-tools %s\n' \
  "$component" "$component_bytes" "$digest" "$rust_toolchain" "$required_wasm_tools_version"
