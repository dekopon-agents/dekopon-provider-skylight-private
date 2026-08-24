# Skylight private provider

> **Exploration only—opt-in, unofficial, private, unsupported, mock-only, not production.** This
> proof of concept is not affiliated with, endorsed by, or supported by Skylight. Skylight publishes
> no public API for these routes; they can change without notice, and using them may violate
> applicable terms or trigger account enforcement. This is not a production integration.

`skylight-private` is a broker-only Rust component implementing API
`dekopon.dev/provider/v1alpha1` with exactly two ordered Medium-risk, read-only, idempotent
capabilities:

| Capability | Description | Fixed request | Projected output |
|---|---|---|---|
| `skylight.private.account.read` | `Reads only the bearer-selected account identifier` | `GET https://app.ourskylight.com/api/user` | `{"account":{"id":"…"}}` |
| `skylight.private.frames.list` | `Lists bounded identifiers and optional names for visible frames` | `GET https://app.ourskylight.com/api/frames` | At most 32 sorted frame IDs and optional marked names |

The manifest description is exactly `Unsupported private Skylight account and frame reads over
broker HTTP`. It has no command words. Both input schemas are exactly
`{"type":"object","properties":{},"additionalProperties":false}`.

## Fixed contract and projection

Only `{}` is accepted. Unknown capability takes precedence over input validation. Every non-object
or non-empty object fails before HTTP, including selectors, pagination, `include`, deleted flags,
endpoint, URL, host, path, method, query, header, body, and credential fields. There is no generic
JSON escape hatch or endpoint override.

Each valid invocation constructs exactly one request and never retries. It is an HTTPS `GET` to the
fixed URI above with an empty body and these ordered guest headers only:

```text
accept: application/json
user-agent: dekopon-skylight-private-provider/0.1 (+https://github.com/dekopon-agents/dekopon)
```

The legacy monorepo URL in the user-agent is intentionally retained as wire contract. The guest
never sets `authorization`, `cookie`, or `content-type`. Only status 200 is decoded. Response
headers and content type are ignored, and response bodies are limited to 262,144 bytes.

Account output retains only `data.id`: a non-empty string of at most 128 UTF-8 bytes. Frame input
must be an object containing array `data`; every record is validated before count or byte
truncation. IDs must be unique, non-empty strings of at most 128 bytes. Missing `attributes` means
unnamed; present `attributes` must be an object. Present `name` and `label`, including `null`, must
be strings. A non-empty name wins, then a non-empty label; otherwise the name is omitted.

Frames sort by ID and retain at most 32 records. Names longer than 256 bytes become the largest
valid UTF-8 prefix plus `…`, within 256 bytes. `nameTruncated` is always present. Whole records are
omitted for count or byte limits; projected JSON is at most 32,640 bytes and the SDK success
envelope remains below 32,768 bytes. Strings are not trimmed, normalized, or character-filtered.
Unknown fields are discarded and malformed known fields fail closed. Retained IDs and names remain
sensitive household metadata; projection is not declassification.

Failures are stable and redact URI, status detail, headers, body, credentials, and transport text:

| Code | Message |
|---|---|
| `unknown-capability` | `unsupported Skylight private capability` |
| `invalid-input` | `input must be exactly an empty object` |
| `invalid-request` | `could not construct the fixed Skylight request` |
| `http-failed` | `broker HTTP request failed` |
| `invalid-response` | `the private API returned an invalid response` |
| `reauth-required` | `the broker credential must be replaced or re-enrolled` |
| `forbidden` | `Skylight refused this private API read` |
| `not-found` | `the private API resource was not found` |
| `rate-limited` | `the private API rate limit was reached` |
| `unexpected-status` | `the private API returned an unexpected status` |

## Required broker-only authority

A deployment must opt in by supplying **both** owner-authored constraint sets below. The duplicate
shape is deliberate: each capability is independently grantable, and neither a manifest import nor
this repository grants authority. The native host follows no redirects. Do not relax the exact
host, method, one-request maximum, HTTPS-only posture, ten-second deadline, or byte ceilings.

```yaml
constraintSets:
  skylight.private.account.read:
    provider: skylight-private
    effect: read-only
    risk: Medium
    idempotency: idempotent
    credential: skylight-poc-bearer
    constraints:
      timeoutMs: 10000
      maxOutputBytes: 32768
      http:
        allowedHosts: [app.ourskylight.com]
        allowedMethods: [GET]
        maxRequests: 1
        maxRequestBytes: 4096
        maxResponseBytes: 262144
        allowPlaintextLoopback: false
  skylight.private.frames.list:
    provider: skylight-private
    effect: read-only
    risk: Medium
    idempotency: idempotent
    credential: skylight-poc-bearer
    constraints:
      timeoutMs: 10000
      maxOutputBytes: 32768
      http:
        allowedHosts: [app.ourskylight.com]
        allowedMethods: [GET]
        maxRequests: 1
        maxRequestBytes: 4096
        maxResponseBytes: 262144
        allowPlaintextLoopback: false
```

The operator must obtain a disposable, short-lived access token out of band, only where authorized,
and install it solely in the owner-only broker credential store. Use a non-PII symbolic name and
bind it to exactly `app.ourskylight.com`. The broker validates the request before injecting
`Authorization: Bearer …` where the guest cannot observe it. Credential values must never enter
source, fixtures, inputs, outputs, errors, logs, traces, evidence, audit, or names.

The credential store is static and loaded at startup. Replacement can require a broker restart.
This component implements no OAuth, login, PKCE, callback, MFA/CAPTCHA handling, refresh, token
cache, rotation, revocation, expiry persistence, enrollment, or pre-expiry renewal. One bearer may
expose multiple accounts or frames; the upstream bearer remains the final resource boundary.

Never add this provider to default catalogs, images, policies, credentials, packages, or
deployments.

## Provenance

This standalone extraction is based on
[`dekopon-agents/dekopon@62d2185f9ec6fee61f2689197b274a9b4947659f`](https://github.com/dekopon-agents/dekopon/commit/62d2185f9ec6fee61f2689197b274a9b4947659f).
The implementation originated in Dekopon PR
[`#120`](https://github.com/dekopon-agents/dekopon/pull/120), squash commit `89dfac98`, with
original branch commits `a853fb26`, `9092095f`, and `e4d5da24`.

The checked monorepo baseline artifact was 246,823 bytes with SHA-256
`1cbb23fd13dc6296e38e360b81c2ce22b73d7605edd81295a05d99d1b8236f0a`. That value records
provenance only; it is not the hash of a standalone release.

Route and response-shape evidence is pinned to
[`joshuaswarren/pyskylight`](https://github.com/joshuaswarren/pyskylight) commit
`69e4576b9035d71aacda9ade7a4afea05a663e94`. The complete upstream MIT notice is in
[`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md). This is a native Rust reimplementation; Python
is not embedded.

## Build and verification

The only compiler provenance is exact Rust 1.89.0. Component composition uses exact `wasm-tools`
1.236.1. All Dekopon dependencies are exact crates.io 0.11.1 pins; there are no Git, path,
symlink, submodule, or adjacent-checkout dependencies. The repository owns only its composed WIT
world. Its two dependency WIT mirrors are checked byte-for-byte against the resolved crates.io
0.11.1 package contents, not trusted by a local hash.

```console
scripts/validate-source.sh
scripts/assert-lock-and-feature-graph.sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --lib
cargo check --locked --target wasm32-unknown-unknown
./build.sh
DEKOPON_SKYLIGHT_COMPONENT=dist/provider-skylight-private.wasm \
  cargo test --locked --test broker_host --test component_host -- --test-threads=1
scripts/verify-component.sh
scripts/test-direct-refusal.sh
scripts/generate-sbom.sh
scripts/check-reproducible.sh
```

`build.sh` writes only ignored files under `target/` and `dist/`: an intermediate core module, the
component, and its checksum. The inventory and CycloneDX SBOM are deterministic generated outputs.
No Wasm, checksum, or SBOM is tracked. There is no release, tag, package, GHCR artifact, deployment,
or supported distribution from this migration.

All behavior tests use synthetic in-memory responses. The component-host test implements the sole
WIT import in memory and opens no socket. The real broker host is used only for pre-network
authority and request-budget refusal; successful native broker HTTP cannot be safely mocked without
changing the fixed production URI. No test contacts Skylight, a public host, DNS, or loopback, and
no captured response fixture is permitted.

The finished component must export only `describe` and `invoke`, import exactly
`dekopon:http/client@1.0.0`, and import no WASI, filesystem, environment, clock, random, socket,
JavaScript, or other ambient interface. The immediate host refuses it because that host provides no
HTTP import. See [`security/RESOURCE_LIMITS.md`](security/RESOURCE_LIMITS.md) for committed ceilings
and measured headroom.
