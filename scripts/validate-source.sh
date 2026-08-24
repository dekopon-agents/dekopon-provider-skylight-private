#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
cd "$root"

expected=$(cat <<'EOF'
.gitattributes
.github/workflows/ci.yml
.github/workflows/release.yml
.gitignore
Cargo.lock
Cargo.toml
LICENSE-APACHE
LICENSE-MIT
README.md
SECURITY.md
THIRD_PARTY_NOTICES.md
build.sh
deny.toml
rust-toolchain.toml
scripts/assert-lock-and-feature-graph.sh
scripts/check-reproducible.sh
scripts/dependency_inventory.py
scripts/generate-sbom.sh
scripts/test-direct-refusal.sh
scripts/validate-source.sh
scripts/verify-component.sh
security/RESOURCE_LIMITS.md
security/wasm-dependencies.txt
src/lib.rs
tests/broker_host.rs
tests/component_host.rs
wit/deps/http.wit
wit/deps/provider.wit
wit/provider.wit
EOF
)
tracked=$(git ls-files | LC_ALL=C sort)
if ! diff -u <(printf '%s\n' "$expected") <(printf '%s\n' "$tracked"); then
  echo 'error: tracked tree differs from the migration specification' >&2
  exit 1
fi

if git ls-files -z | grep -zEq '(^|/)(target|dist)/|\.wasm(\.sha256)?$|\.cdx\.json$'; then
  echo 'error: generated Wasm/build/SBOM output is tracked' >&2
  exit 1
fi
if find . -path ./.git -prune -o -type l -print -quit | grep -q .; then
  echo 'error: symlinks are not allowed' >&2
  exit 1
fi

python3 - "$root" <<'PY'
from pathlib import Path
import hashlib, re, sys, tomllib
root = Path(sys.argv[1])
expected_wit = """package dekopon:skylight-private@0.1.0;

world provider {
    include dekopon:provider/provider@0.2.0;
    import dekopon:http/client@1.0.0;
}
"""
if (root / "wit/provider.wit").read_text() != expected_wit:
    raise SystemExit("error: caller-owned composed world is not exact")
manifest = tomllib.loads((root / "Cargo.toml").read_text())
package = manifest["package"]
required = {
    "name": "dekopon-skylight-private-provider",
    "version": "0.1.0",
    "edition": "2024",
    "rust-version": "1.89.0",
    "repository": "https://github.com/dekopon-agents/dekopon-provider-skylight-private",
    "publish": False,
}
for key, value in required.items():
    if package.get(key) != value:
        raise SystemExit(f"error: Cargo package field {key} is not {value!r}")
if set(manifest.get("workspace", {})):
    raise SystemExit("error: [workspace] must remain empty")
source = (root / "src/lib.rs").read_text()
required_source = [
    'const ACCOUNT_URI: &str = "https://app.ourskylight.com/api/user";',
    'const FRAMES_URI: &str = "https://app.ourskylight.com/api/frames";',
    '"dekopon-skylight-private-provider/0.1 (+https://github.com/dekopon-agents/dekopon)";',
    'const MAX_RESPONSE_BODY_BYTES: usize = 256 * 1024;',
    'const MAX_COMPONENT_OUTPUT_BYTES: usize = 32 * 1024;',
]
for needle in required_source:
    if source.count(needle) != 1:
        raise SystemExit(f"error: fixed source contract missing or duplicated: {needle}")
legacy_tests = {
    "manifest_is_exactly_the_two_medium_read_capabilities": "5f99b9e474a08c822a5dc2f75a7153d9fe1e08e886b4b58578422444cacdefa0",
    "unknown_non_object_and_extra_field_inputs_never_send": "07c09cba450492cfd789481c29673348ea10b4a2def3604f7bfc02cf75bf8cae",
    "account_uses_one_exact_fixed_request_and_projects_only_the_id": "df2a8093fa2ef6e44f716f40737cee01757cee918fb65975836c1a874d1e9795",
    "account_rejects_missing_empty_non_string_and_oversized_ids": "dfaff88574be531203cdeacb6c1ab0f37901a259f60e27813fd2a8f4f9fe5abd",
    "frames_use_one_exact_fixed_request_and_project_sorted_names": "427a400e2e7d99e6f4f76ccd8ec12e48a0826167a54ff5e7e311ee0aa8ce9ded",
    "frames_reject_duplicate_and_malformed_identities": "666ffcec4a9d5ea9966c60f750a832087b08ad552ced4cb7101b704c5c3ae445",
    "frames_reject_invalid_envelopes_and_wrong_known_field_types": "e880d56e330592acd9e67443ab58be03fd0232f8cfae1f8ab3cd93c9e3f66d2e",
    "an_empty_frame_array_is_a_valid_complete_projection": "37e2990f71da592087c9de3cfb30a0f2b864b29988de7d536618a53a4fcf14c5",
    "frame_count_is_capped_after_stable_id_sorting": "e25b777d7e53cedec9ddf11131b3e1d61ab248b4c8343820fe6d6ef2f5c0e871",
    "frame_output_budget_omits_whole_records_and_marks_truncation": "b366bc0f8cef9a3cab40b5ee0d1d238de2b366af4091d6589557937235c23b4e",
    "frame_names_are_utf8_safe_and_include_the_truncation_marker": "c121e254f70ee7114558128b5f9aaf3b31fa1c273549c78c69e8bacf1abfd03d",
    "private_and_secret_sentinel_fields_never_enter_frame_output": "f8a379b4364c9a2f2c5195ce3c335e5fe63e70ab7c6eedaca9b1dd346e193a55",
    "statuses_and_invalid_bodies_map_to_stable_failures_without_retry": "3095a044301e1f7eebd9e9c711593415a6f6afce9473ac5e18fd9c1ea117b923",
    "every_transport_failure_collapses_raw_detail_to_bounded_http_failed": "fc5dcc0fad2bca4672f86102a835706b67737030d5f6db60cfec7d8b3a082f25",
}
for name, expected_hash in legacy_tests.items():
    marker = f"    #[test]\n    fn {name}()"
    if source.count(marker) != 1:
        raise SystemExit(f"error: legacy inline test is missing or duplicated: {name}")
    start = source.index(marker)
    end = source.find("\n    #[test]\n", start + len(marker))
    if end < 0:
        end = source.rfind("\n}")
    definition = source[start:end].rstrip().encode()
    if hashlib.sha256(definition).hexdigest() != expected_hash:
        raise SystemExit(f"error: legacy inline test changed: {name}")
readme = (root / "README.md").read_text()
for provenance in [
    "62d2185f9ec6fee61f2689197b274a9b4947659f",
    "89dfac98", "a853fb26", "9092095f", "e4d5da24",
    "246,823", "1cbb23fd13dc6296e38e360b81c2ce22b73d7605edd81295a05d99d1b8236f0a",
    "69e4576b9035d71aacda9ade7a4afea05a663e94",
]:
    if provenance not in readme:
        raise SystemExit(f"error: README provenance is missing {provenance}")
for workflow in (root / ".github/workflows").glob("*.yml"):
    text = workflow.read_text()
    for match in re.finditer(r"^\s*uses:\s*([^\s#]+)", text, re.MULTILINE):
        use = match.group(1)
        if use.startswith("./"):
            continue
        if not re.search(r"@[0-9a-f]{40}$", use):
            raise SystemExit(f"error: action is not full-SHA pinned in {workflow.name}: {use}")
release = (root / ".github/workflows/release.yml").read_text()
for forbidden in ("workflow_dispatch", "workflow_call", "branches:"):
    if forbidden in release:
        raise SystemExit(f"error: dormant release workflow contains forbidden trigger {forbidden}")
if "tags:\n      - \"v0.1.0\"" not in release:
    raise SystemExit("error: release trigger is not the exact future v0.1.0 tag")
if release.count("git fetch --force origin") != 3:
    raise SystemExit("error: every release job must force-fetch the annotated tag object")
if release.count("contents: write") != 1 or release.count("packages: write") != 1:
    raise SystemExit("error: draft and GHCR publication permissions must remain split")
for invariant in [
    "needs:\n      - gates\n      - draft",
    "indeterminate GHCR lookup; refusing to publish",
    "remote manifest differs from the exact deterministic manifest",
    "verified-assets.tsv",
]:
    if invariant not in release:
        raise SystemExit(f"error: idempotent release invariant is missing: {invariant}")
PY

if grep -REn --include='Cargo.toml' --include='Cargo.lock' \
  '(^|[[:space:]{])(path|git)[[:space:]]*=' .; then
  echo 'error: local or Git Cargo dependency found' >&2
  exit 1
fi
if git grep -En '/Users/|/home/[^ ]+/|file://|\.\./dekopon|\.worktrees/' -- ':!scripts/validate-source.sh'; then
  echo 'error: tracked source contains a local or adjacent-checkout path' >&2
  exit 1
fi
if git grep -En '(gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{30,}|AKIA[0-9A-Z]{16}|-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----)'; then
  echo 'error: tracked source resembles a credential or private key' >&2
  exit 1
fi

python3 scripts/dependency_inventory.py --check
echo 'exact tracked source tree, fixed contract, provenance, workflows, paths, and secret scan verified'
