# Resource limits

These ceilings are part of the unsupported exploration contract. Both owner-authored capability
constraint sets use the same values, but remain independently grantable.

| Resource | Committed ceiling | Enforcement/evidence |
|---|---:|---|
| Guest linear memory | 32 MiB (33,554,432 bytes) | Wasmtime limiter in component and broker-host tests |
| Guest fuel | 128,000,000 | Fresh-store fuel in component and broker-host tests |
| Composed component | 393,216 bytes | `build.sh`, integration tests, and component inspection |
| Serialized input | 4,096 bytes | Broker host limit; valid provider input is only two-byte `{}` |
| Accounted HTTP request | 4,096 bytes | Owner grant and broker host limit |
| Buffered HTTP response | 262,144 bytes | Owner grant, broker host limit, and guest check |
| SDK response envelope | 32,768 bytes (strictly below) | Guest reserve plus SDK-type and component-host serialization tests |
| Wall-clock duration | 10 seconds | Owner grant and broker host maximum |
| HTTP calls | 1 | Owner grant, broker host maximum, `FnOnce` guest boundary, and request assertions |

`allowPlaintextLoopback` is false and the exact host is `app.ourskylight.com`; neither is relaxed for
tests. The real broker integration proves nonmatching authority and undersized-request grants fail
before dispatch with empty HTTP evidence. Success is exercised only through the real component and
an in-memory implementation of its sole WIT import.

## Measured headroom

Measurements below came from the pinned Rust 1.89.0 / wasm-tools 1.236.1 revision build and its
socket-free in-memory component host. They are not production capacity claims. Time is
machine-dependent; byte, memory-page, and fuel measurements are pinned by the tests.

| Measurement | Observed | Headroom to committed ceiling |
|---|---:|---:|
| Composed component bytes | 291,152 bytes | 102,064 bytes |
| Worst-case actual SDK frame envelope (13 whole records retained) | 30,430 bytes | 2,338 bytes |
| Near-limit valid SDK envelope (32 smallest records retained) | 1,182 bytes | 31,586 bytes |
| Near-limit valid response body | 260,010 bytes | 2,134 bytes |
| Near-limit malformed-last response body | 260,049 bytes | 2,095 bytes |
| Peak guest memory requested by either near-limit probe | 2,359,296 bytes | 31,195,136 bytes |
| Fuel: near-limit valid response | 64,652,737 | 63,347,263 |
| Fuel: near-limit malformed-last duplicate | 71,150,193 | 56,849,807 |
| In-memory near-limit test elapsed time | 2,907 ms | 7,093 ms |

The smaller output-budget fixture is generated in memory from synthetic control-character-heavy IDs
and names. The near-limit probes generate 20,000 unique three-byte UTF-8 IDs in a deterministic
permutation; the hostile variant puts duplicate known `name` members in the final record. None is
captured Skylight data. The guest validates every record and tracks every ID for duplicate detection,
but retains only the 32 smallest projected records. Whole frame records are omitted before the SDK
envelope can reach 32,768 bytes. Host allocations outside guest linear memory are not represented by
the guest-memory measurement, so the 32 MiB value remains a hard Wasmtime ceiling rather than a
general process-RSS claim.
