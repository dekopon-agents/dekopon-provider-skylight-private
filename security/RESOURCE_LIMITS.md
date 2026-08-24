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

Measurements below came from the pinned Rust 1.89.0 / wasm-tools 1.236.1 standalone build and the
synthetic worst-case component-host test before the migration commit. They are not production
capacity claims. Time is machine-dependent; byte, memory-page, and fuel measurements are pinned by
the test.

| Measurement | Observed | Headroom to committed ceiling |
|---|---:|---:|
| Composed component bytes | 295,850 bytes | 97,366 bytes |
| Worst-case actual SDK frame envelope (13 whole records retained) | 30,430 bytes | 2,338 bytes |
| Peak guest memory requested in the in-memory host | 1,310,720 bytes | 32,243,712 bytes |
| Fuel consumed by instantiate + describe + account + worst-case frames | 8,412,897 | 119,587,103 |
| In-memory test elapsed time | 3 ms | 9,997 ms |
| Synthetic worst-case response body | 78,986 bytes | 183,158 bytes |

The response fixture is generated in memory from synthetic control-character-heavy IDs and names;
it is not captured Skylight data. Whole frame records are omitted before the SDK envelope can reach
32,768 bytes. Host allocations outside guest linear memory are not represented by the guest-memory
measurement, so the 32 MiB value remains a hard Wasmtime ceiling rather than a general process-RSS
claim.
