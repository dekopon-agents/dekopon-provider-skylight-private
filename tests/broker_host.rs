//! Broker-authorized real-component refusal tests. No call may reach a private host.
use std::{
    io::{Read as _, Write as _},
    net::TcpListener,
    os::unix::net::UnixStream,
    path::PathBuf,
    thread::JoinHandle,
    time::Duration,
};

use dekopon_broker_host::{
    BoundCredential, BrokerHostError, BrokerHostLimits, BrokerInvocationFailure,
    BrokerInvocationOutput, BrokerProviderRegistry, HostCallRejection, Streams, TestImports,
    asset::AssetInputs,
};
use dekopon_capability::{
    AuthorizedInvocation, ChatSlotValues, ExecutionConstraints, HttpConstraints,
    ProposedInvocation, broker::AuthorizationGate,
};
use dekopon_core::{Actor, AgentId, InvocationId, PrincipalId, Redacted, TraceId};
use dekopon_http_host::{LoopbackHttpsPin, PolicyViolation};
use dekopon_provider_sdk::EffectKind;
use serde_json::{Value, json};

fn limits() -> BrokerHostLimits {
    BrokerHostLimits {
        max_memory_bytes: 32 * 1024 * 1024,
        max_input_bytes: 4096,
        max_output_bytes: 32 * 1024,
        max_http_requests: 1,
        max_http_request_bytes: 4096,
        max_http_response_bytes: 262_144,
        fuel: 128_000_000,
        max_timeout: Duration::from_secs(10),
        ..BrokerHostLimits::default()
    }
}

fn component() -> PathBuf {
    std::env::var_os("DEKOPON_PROVIDER_COMPONENT")
        .expect("build the component first")
        .into()
}

#[test]
fn sdk_component_fits_one_mib_and_preserves_broker_resource_limits() {
    let bytes = std::fs::metadata(component())
        .expect("built component")
        .len();
    assert!(
        bytes <= 1_048_576,
        "typed stdio component is {bytes} bytes; limit is 1 MiB"
    );
    let limits = limits();
    assert_eq!(limits.max_memory_bytes, 32 * 1024 * 1024);
    assert_eq!(limits.fuel, 128_000_000);
    assert_eq!(limits.max_input_bytes, 4096);
    assert_eq!(limits.max_output_bytes, 32 * 1024);
    assert_eq!(limits.max_http_requests, 1);
    assert_eq!(limits.max_http_request_bytes, 4096);
    assert_eq!(limits.max_http_response_bytes, 262_144);
    assert_eq!(limits.max_timeout, Duration::from_secs(10));
}

#[tokio::test(flavor = "multi_thread")]
async fn real_component_fuel_exhaustion_refuses_even_description_before_http() {
    let error = match BrokerProviderRegistry::load(
        [component()],
        BrokerHostLimits {
            fuel: 1,
            ..limits()
        },
    )
    .await
    {
        Ok(_) => panic!("one fuel unit cannot load the component"),
        Err(error) => error,
    };
    assert!(matches!(error, BrokerHostError::Instantiate { .. }));
    assert!(format!("{error:?}").contains("all fuel consumed"));
    // The broker never constructed an authorized invocation or connected to a host.
}
fn grant(authority: &str, method: &str, bytes: u64) -> ExecutionConstraints {
    ExecutionConstraints {
        timeout_ms: 10_000,
        http: Some(HttpConstraints {
            allowed_hosts: vec![authority.to_owned()],
            allowed_methods: vec![method.to_owned()],
            max_requests: 1,
            max_request_bytes: bytes,
            max_response_bytes: 262_144,
            allow_plaintext_loopback: false,
            propagate_trace: false,
            request_templates: Vec::new(),
            signed_download: None,
        }),
        ..ExecutionConstraints::default()
    }
}
fn authorized(
    capability: &str,
    input: Value,
    constraints: ExecutionConstraints,
) -> AuthorizedInvocation {
    let proposal = ProposedInvocation::new(
        "skylight-test-invocation".parse::<InvocationId>().unwrap(),
        capability.parse().unwrap(),
        Actor::Agent {
            agent: "skylight-test-agent".parse::<AgentId>().unwrap(),
        },
        "5ce1a6207e57000000000000decafbad"
            .parse::<TraceId>()
            .unwrap(),
        input,
    );
    AuthorizationGate::new()
        .authorize(
            proposal,
            "skylight-private".parse().unwrap(),
            "skylight-test-decision".to_owned(),
            "skylight-test-broker".parse::<PrincipalId>().unwrap(),
            // 0.75.0 requires a 64-hex-digit policy digest; a synthetic one, as the testkit uses.
            "a".repeat(64),
            constraints,
        )
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn each_read_requires_the_exact_host_method_budget_and_destination_bound_secret() {
    let registry = BrokerProviderRegistry::load([component()], limits())
        .await
        .unwrap();
    let cases = [
        ("skylight-private.account.read", json!({})),
        ("skylight-private.frames.list", json!({})),
        (
            "skylight-private.categories.list",
            json!({"frameId":"frame-test"}),
        ),
        (
            "skylight-private.calendar.events.list",
            json!({"frameId":"frame-test","dateMin":"2028-03-11","dateMax":"2028-03-13","timezone":"America/New_York"}),
        ),
        (
            "skylight-private.lists.list",
            json!({"frameId":"frame-test"}),
        ),
        (
            "skylight-private.lists.read",
            json!({"frameId":"frame-test","listId":"list-test"}),
        ),
        (
            "skylight-private.list.items.list",
            json!({"frameId":"frame-test","listId":"list-test"}),
        ),
        (
            "skylight-private.tasks.list",
            json!({"frameId":"frame-test","after":"2028-03-11","before":"2028-03-11"}),
        ),
        (
            "skylight-private.messages.list",
            json!({"frameId":"frame-test"}),
        ),
        (
            "skylight-private.messages.read",
            json!({"frameId":"frame-test","messageId":"message-test"}),
        ),
        (
            "skylight-private.messages.likes.list",
            json!({"frameId":"frame-test","messageId":"message-test"}),
        ),
        (
            "skylight-private.messages.comments.list",
            json!({"frameId":"frame-test","messageId":"message-test"}),
        ),
        (
            "skylight-private.albums.list",
            json!({"frameId":"frame-test"}),
        ),
        (
            "skylight-private.albums.messages.list",
            json!({"frameId":"frame-test","albumId":"album-test"}),
        ),
        (
            "skylight-private.albums.message-ids.list",
            json!({"frameId":"frame-test","albumId":"album-test"}),
        ),
        (
            "skylight-private.month-in-review.read",
            json!({"frameId":"frame-test"}),
        ),
        (
            "skylight-private.month-in-review.list",
            json!({"frameId":"frame-test"}),
        ),
    ];
    assert_eq!(
        registry
            .manifests()
            .next()
            .unwrap()
            .capabilities
            .iter()
            .filter(|capability| capability.effect == EffectKind::ReadOnly)
            .count(),
        cases.len()
    );
    for (capability, input) in cases {
        for (host, method, bytes, reason) in [
            ("not-skylight.invalid", "GET", 4096, PolicyViolation::Denied),
            (
                "app.ourskylight.com.evil.invalid",
                "GET",
                4096,
                PolicyViolation::Denied,
            ),
            (
                "app.ourskylight.com:444",
                "GET",
                4096,
                PolicyViolation::Denied,
            ),
            ("app.ourskylight.com", "POST", 4096, PolicyViolation::Denied),
            ("app.ourskylight.com", "GET", 1, PolicyViolation::ByteLimit),
        ] {
            let failure = registry
                .invoke(
                    authorized(capability, input.clone(), grant(host, method, bytes)),
                    None,
                    AssetInputs::default(),
                )
                .await
                .unwrap_err();
            assert!(
                matches!(failure.error.as_ref(), BrokerHostError::HostCallRejected { reason: HostCallRejection::Http(actual), .. } if *actual == reason),
                "{capability}: {:?}",
                failure.error
            );
            assert!(
                failure.http_calls.is_empty(),
                "no HTTP effect before the grant"
            );
        }
        let credential = BoundCredential::bearer(
            "Bearer",
            Redacted::new("synthetic-credential-sentinel".to_owned()),
            vec!["not-skylight.invalid".to_owned()],
        )
        .unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let authority = format!("fixture.example.test:{}", address.port());
        let imports = TestImports {
            loopback_https_pin: Some(
                LoopbackHttpsPin::new(&authority, address, b"synthetic-unused-trust-root".to_vec())
                    .unwrap(),
            ),
            settings: Some(json!({"baseUrl":format!("https://{authority}/skylight")}).to_string()),
            credential: Some(credential),
            ..TestImports::default()
        };
        let failure = registry
            .invoke_with_test_imports(
                authorized(capability, input, grant(&authority, "GET", 4096)),
                ChatSlotValues::default(),
                AssetInputs::default(),
                Some(&imports),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(
                failure.error.as_ref(),
                BrokerHostError::HostCallRejected {
                    reason: HostCallRejection::Http(PolicyViolation::Denied),
                    ..
                }
            ),
            "{capability}: {:?}",
            failure.error
        );
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        assert_eq!(failure.http_calls.len(), 1);
        assert!(!failure.http_calls[0].credential_injected);
        assert_eq!(failure.http_calls[0].status, None);
        assert!(!format!("{failure:?}").contains("synthetic-credential-sentinel"));
    }
}

const EVENTS_TARGET: &str = "/skylight/api/frames/frame-test/calendar_events";
const LISTS_TARGET: &str = "/skylight/api/frames/frame-test/lists";
const ITEMS_TARGET: &str = "/skylight/api/frames/frame-test/lists/list-test/list_items";
/// The same bytes the native create test pins (`src/household_tests.rs`).
const CREATE_BODY: &str = r#"{"summary":"Dentist","starts_at":"2028-03-11T09:00:00-05:00","ends_at":"2028-03-11T10:00:00-05:00","all_day":false,"timezone":"America/New_York","category_ids":["category-1","category-2"],"rrule":["RRULE:FREQ=WEEKLY;COUNT=2"]}"#;

fn create_input() -> Value {
    json!({
        "frameId": "frame-test",
        "summary": "Dentist",
        "startsAt": "2028-03-11T09:00:00-05:00",
        "endsAt": "2028-03-11T10:00:00-05:00",
        "allDay": false,
        "timezone": "America/New_York",
        "categoryIds": ["category-1", "category-2"],
        "rrule": ["RRULE:FREQ=WEEKLY;COUNT=2"]
    })
}

fn writes() -> [(&'static str, &'static str, Value, String); 10] {
    [
        (
            "skylight-private.calendar.events.create",
            "POST",
            create_input(),
            EVENTS_TARGET.to_owned(),
        ),
        (
            "skylight-private.calendar.events.update",
            "PUT",
            json!({"frameId":"frame-test","eventId":"event-7","summary":"Dentist moved"}),
            format!("{EVENTS_TARGET}/event-7"),
        ),
        (
            "skylight-private.calendar.events.delete",
            "DELETE",
            json!({"frameId":"frame-test","eventId":"event-7"}),
            format!("{EVENTS_TARGET}/event-7"),
        ),
        (
            "skylight-private.list.items.create",
            "POST",
            json!({"frameId":"frame-test","listId":"list-test","label":"Milk"}),
            ITEMS_TARGET.to_owned(),
        ),
        (
            "skylight-private.list.items.update",
            "PUT",
            json!({"frameId":"frame-test","listId":"list-test","itemId":"item-7","position":2}),
            format!("{ITEMS_TARGET}/item-7"),
        ),
        (
            "skylight-private.list.items.complete",
            "PUT",
            json!({"frameId":"frame-test","listId":"list-test","itemId":"item-7"}),
            format!("{ITEMS_TARGET}/item-7"),
        ),
        (
            "skylight-private.list.items.delete",
            "DELETE",
            json!({"frameId":"frame-test","listId":"list-test","itemId":"item-7"}),
            format!("{ITEMS_TARGET}/item-7"),
        ),
        (
            "skylight-private.lists.create",
            "POST",
            json!({"frameId":"frame-test","label":"Groceries"}),
            LISTS_TARGET.to_owned(),
        ),
        (
            "skylight-private.lists.update",
            "PUT",
            json!({"frameId":"frame-test","listId":"list-test","kind":"to_do"}),
            format!("{LISTS_TARGET}/list-test"),
        ),
        (
            "skylight-private.lists.delete",
            "DELETE",
            json!({"frameId":"frame-test","listId":"list-test"}),
            format!("{LISTS_TARGET}/list-test"),
        ),
    ]
}

/// One request exactly as the loopback consumer received it.
struct Captured {
    request_line: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

/// A plaintext loopback consumer that keeps the complete request and answers one fixed response.
/// The broker reaches it only under `allow_plaintext_loopback`; the guest's bytes do not depend on
/// the scheme, which comes from the owner `baseUrl` setting.
fn loopback_consumer(
    status: &'static str,
    body: &'static str,
) -> (TcpListener, JoinHandle<Captured>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let server = listener.try_clone().unwrap();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = server.accept().unwrap();
        let mut bytes = Vec::new();
        let mut chunk = [0; 4096];
        let head_end = loop {
            let read = stream.read(&mut chunk).unwrap();
            assert!(read > 0, "request ended before its headers");
            bytes.extend_from_slice(&chunk[..read]);
            if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                break end;
            }
        };
        let head = String::from_utf8(bytes[..head_end].to_vec()).unwrap();
        let mut lines = head.split("\r\n");
        let request_line = lines.next().unwrap().to_owned();
        let headers: Vec<(String, String)> = lines
            .map(|line| {
                let (name, value) = line.split_once(':').unwrap();
                (name.trim().to_ascii_lowercase(), value.trim().to_owned())
            })
            .collect();
        let length = headers
            .iter()
            .find(|(name, _)| name == "content-length")
            .map_or(0, |(_, value)| value.parse::<usize>().unwrap());
        let mut body_bytes = bytes[head_end + 4..].to_vec();
        while body_bytes.len() < length {
            let read = stream.read(&mut chunk).unwrap();
            assert!(read > 0, "request ended before its body");
            body_bytes.extend_from_slice(&chunk[..read]);
        }
        write!(
            stream,
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        Captured {
            request_line,
            headers,
            body: body_bytes,
        }
    });
    (listener, handle)
}

fn loopback_grant(port: u16, method: &str) -> ExecutionConstraints {
    let mut constraints = grant(&format!("127.0.0.1:{port}"), method, 4096);
    constraints.http.as_mut().unwrap().allow_plaintext_loopback = true;
    constraints
}

/// Invokes the built component with captured stdout, as the broker would for an agent.
async fn invoke_loopback(
    registry: &BrokerProviderRegistry,
    capability: &str,
    input: Value,
    constraints: ExecutionConstraints,
    port: u16,
) -> (
    Result<BrokerInvocationOutput, BrokerInvocationFailure>,
    Vec<u8>,
) {
    let (host_end, mut reader) = UnixStream::pair().unwrap();
    let stdout = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let imports = TestImports {
        settings: Some(json!({"baseUrl":format!("http://127.0.0.1:{port}/skylight")}).to_string()),
        ..TestImports::default()
    };
    let result = registry
        .invoke_with_test_imports(
            authorized(capability, input, constraints),
            ChatSlotValues::default(),
            AssetInputs {
                streams: Some(Streams {
                    stdin: None,
                    stdout: host_end.into(),
                }),
                ..AssetInputs::default()
            },
            Some(&imports),
        )
        .await;
    (result, stdout.join().unwrap())
}

fn assert_no_connection(listener: &TcpListener) {
    listener.set_nonblocking(true).unwrap();
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock,
        "the consumer saw a connection"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn smoke_the_built_component_posts_the_exact_event_body_and_a_get_only_grant_refuses_it() {
    let registry = BrokerProviderRegistry::load([component()], limits())
        .await
        .unwrap();
    let capability = "skylight-private.calendar.events.create";
    let (listener, consumer) = loopback_consumer(
        "201 Created",
        r#"{"data":{"id":"event-synthetic","type":"calendar_event","attributes":{"summary":"Dentist"}}}"#,
    );
    let port = listener.local_addr().unwrap().port();
    let (result, stdout) = invoke_loopback(
        &registry,
        capability,
        create_input(),
        loopback_grant(port, "POST"),
        port,
    )
    .await;
    let output = result.expect("a POST-only grant accepts the create");
    let captured = consumer.join().unwrap();
    assert_no_connection(&listener);
    assert_eq!(output.http_calls.len(), 1);
    assert_eq!(output.http_calls[0].method, "POST");
    assert_eq!(output.http_calls[0].status, Some(201));
    assert!(!output.http_calls[0].credential_injected);
    assert_eq!(
        captured.request_line,
        format!("POST {EVENTS_TARGET} HTTP/1.1")
    );
    for (name, value) in [
        ("accept", "application/json"),
        (
            "user-agent",
            "dekopon-skylight-private-provider/0.1 (+https://github.com/dekopon-agents/dekopon)",
        ),
        ("skylight-api-version", "2026-05-01"),
        ("content-type", "application/json"),
    ] {
        let sent: Vec<_> = captured
            .headers
            .iter()
            .filter(|(header, _)| header == name)
            .map(|(_, value)| value.as_str())
            .collect();
        assert_eq!(sent, [value], "{name}");
    }
    for denied in ["authorization", "cookie"] {
        assert!(captured.headers.iter().all(|(name, _)| name != denied));
    }
    assert_eq!(
        String::from_utf8(captured.body.clone()).unwrap(),
        CREATE_BODY
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&captured.body).unwrap(),
        json!({
            "summary": "Dentist",
            "starts_at": "2028-03-11T09:00:00-05:00",
            "ends_at": "2028-03-11T10:00:00-05:00",
            "all_day": false,
            "timezone": "America/New_York",
            "category_ids": ["category-1", "category-2"],
            "rrule": ["RRULE:FREQ=WEEKLY;COUNT=2"]
        })
    );
    let projected: Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(projected["event"]["id"], "event-synthetic");

    // Today's deployment allows GET only: the same call is refused before any HTTP.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (result, _) = invoke_loopback(
        &registry,
        capability,
        create_input(),
        loopback_grant(port, "GET"),
        port,
    )
    .await;
    let failure = result.expect_err("a GET-only grant refuses the create");
    assert!(
        matches!(
            failure.error.as_ref(),
            BrokerHostError::HostCallRejected {
                reason: HostCallRejection::Http(PolicyViolation::Denied),
                ..
            }
        ),
        "{:?}",
        failure.error
    );
    assert!(failure.http_calls.is_empty());
    assert_no_connection(&listener);
}

#[tokio::test(flavor = "multi_thread")]
async fn each_write_runs_only_under_its_exact_method_and_never_under_a_get_only_grant() {
    let registry = BrokerProviderRegistry::load([component()], limits())
        .await
        .unwrap();
    let manifest = registry.manifests().next().unwrap();
    for (capability, method, input, target) in writes() {
        let declared = manifest
            .capabilities
            .iter()
            .find(|declared| declared.id.as_str() == capability)
            .unwrap();
        assert_eq!(declared.effect, EffectKind::ExternalWrite, "{capability}");
        for other in ["GET", "POST", "PUT", "PATCH", "DELETE"] {
            if other == method {
                continue;
            }
            let failure = registry
                .invoke(
                    authorized(
                        capability,
                        input.clone(),
                        grant("app.ourskylight.com", other, 4096),
                    ),
                    None,
                    AssetInputs::default(),
                )
                .await
                .unwrap_err();
            assert!(
                matches!(
                    failure.error.as_ref(),
                    BrokerHostError::HostCallRejected {
                        reason: HostCallRejection::Http(PolicyViolation::Denied),
                        ..
                    }
                ),
                "{capability} under {other}: {:?}",
                failure.error
            );
            assert!(failure.http_calls.is_empty(), "{capability} under {other}");
        }
        let (listener, consumer) = loopback_consumer("204 No Content", "");
        let port = listener.local_addr().unwrap().port();
        let (result, stdout) = invoke_loopback(
            &registry,
            capability,
            input,
            loopback_grant(port, method),
            port,
        )
        .await;
        let output = result.unwrap_or_else(|failure| panic!("{capability}: {:?}", failure.error));
        let captured = consumer.join().unwrap();
        assert_eq!(output.http_calls.len(), 1);
        assert_eq!(captured.request_line, format!("{method} {target} HTTP/1.1"));
        assert!(
            captured
                .headers
                .contains(&("content-type".to_owned(), "application/json".to_owned()))
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&stdout).unwrap(),
            json!({"ok": true})
        );
    }
}
