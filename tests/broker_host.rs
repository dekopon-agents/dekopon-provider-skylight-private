use std::{path::PathBuf, time::Duration};

use dekopon_broker_host::{BrokerHostError, BrokerHostLimits, BrokerProviderRegistry};
use dekopon_capability::{
    AuthorizedInvocation, EffectKind, ExecutionConstraints, HttpConstraints, ProposedInvocation,
    broker::AuthorizationGate,
};
use dekopon_core::{Actor, AgentId, InvocationId, PrincipalId, Redacted, RiskLevel, TraceId};
use dekopon_provider_sdk::ProviderApiVersion;
use serde_json::json;

mod household_cases;

const MAX_COMPONENT_BYTES: u64 = 589_824;
const MAX_MEMORY_BYTES: usize = 32 * 1024 * 1024;
const MAX_FUEL: u64 = 128_000_000;
const MAX_INPUT_BYTES: usize = 4_096;
const MAX_REQUEST_BYTES: u64 = 4_096;
const MAX_RESPONSE_BYTES: u64 = 262_144;
const MAX_OUTPUT_BYTES: u64 = 32_768;
const TIMEOUT_MS: u64 = 10_000;

fn component_path() -> PathBuf {
    PathBuf::from(
        std::env::var_os("DEKOPON_PROVIDER_COMPONENT")
            .expect("DEKOPON_PROVIDER_COMPONENT must point at the built component"),
    )
}

fn host_limits() -> BrokerHostLimits {
    BrokerHostLimits {
        max_memory_bytes: MAX_MEMORY_BYTES,
        max_input_bytes: MAX_INPUT_BYTES,
        max_output_bytes: MAX_OUTPUT_BYTES as usize,
        max_http_requests: 1,
        max_http_request_bytes: MAX_REQUEST_BYTES,
        max_http_response_bytes: MAX_RESPONSE_BYTES,
        fuel: MAX_FUEL,
        max_timeout: Duration::from_millis(TIMEOUT_MS),
        ..BrokerHostLimits::default()
    }
}

fn constraints(authority: &str, max_request_bytes: u64) -> ExecutionConstraints {
    ExecutionConstraints {
        timeout_ms: TIMEOUT_MS,
        max_output_bytes: MAX_OUTPUT_BYTES,
        http: Some(HttpConstraints {
            allowed_hosts: vec![authority.to_owned()],
            allowed_methods: vec!["GET".to_owned()],
            max_requests: 1,
            max_request_bytes,
            max_response_bytes: MAX_RESPONSE_BYTES,
            allow_plaintext_loopback: false,
        }),
        storage: None,
        secret_use: None,
    }
}

fn authorized(capability: &str, constraints: ExecutionConstraints) -> AuthorizedInvocation {
    authorized_input(capability, json!({}), constraints)
}

fn authorized_input(
    capability: &str,
    input: serde_json::Value,
    constraints: ExecutionConstraints,
) -> AuthorizedInvocation {
    let capability = capability.parse().expect("valid capability fixture");
    let proposal = ProposedInvocation::new(
        "skylight-test-invocation"
            .parse::<InvocationId>()
            .expect("valid invocation fixture"),
        capability,
        Actor::Agent {
            agent: "skylight-test-agent"
                .parse::<AgentId>()
                .expect("valid agent fixture"),
        },
        // 0.13.0 made `TraceId` the W3C one: sixteen bytes, thirty-two lowercase hex digits, so a
        // free-form fixture name no longer parses. These digits are synthetic and correlate nothing.
        "5ce1a6207e57000000000000decafbad"
            .parse::<TraceId>()
            .expect("valid trace fixture"),
        input,
    );
    AuthorizationGate::new()
        .authorize(
            proposal,
            "skylight-private".parse().expect("valid provider fixture"),
            "skylight-test-decision".to_owned(),
            "skylight-test-broker"
                .parse::<PrincipalId>()
                .expect("valid broker fixture"),
            "skylight-test-policy".to_owned(),
            constraints,
        )
        .expect("fixture authorization is structurally valid")
}

async fn load() -> BrokerProviderRegistry {
    let component = component_path();
    let bytes = std::fs::metadata(&component)
        .unwrap_or_else(|error| panic!("build {} first: {error}", component.display()))
        .len();
    assert!(
        bytes <= MAX_COMPONENT_BYTES,
        "component is {bytes} bytes; maximum is {MAX_COMPONENT_BYTES}"
    );
    BrokerProviderRegistry::load([component], host_limits())
        .await
        .expect("the broker host loads the HTTP-importing component without description-time I/O")
}

#[tokio::test(flavor = "multi_thread")]
async fn crates_io_broker_loads_the_exact_manifest() {
    let registry = load().await;
    let manifest = registry.manifests().next().expect("one manifest is loaded");
    assert_eq!(manifest.api_version, ProviderApiVersion::V1Alpha1);
    assert_eq!(manifest.id.as_str(), "skylight-private");
    assert_eq!(
        manifest.description,
        "Unsupported private Skylight account and frame reads over broker HTTP"
    );
    assert_eq!(manifest.command_words, ["skylight"]);
    assert_eq!(manifest.capabilities.len(), 8);

    let expected = [
        (
            "skylight.private.account.read",
            "Reads only the bearer-selected account identifier",
        ),
        (
            "skylight.private.frames.list",
            "Lists bounded identifiers and optional names for visible frames",
        ),
    ];
    let schema = json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false
    });
    for (capability, (id, description)) in manifest.capabilities.iter().zip(expected) {
        assert_eq!(capability.id.as_str(), id);
        assert_eq!(capability.description, description);
        assert_eq!(capability.effect, EffectKind::ReadOnly);
        assert_eq!(capability.risk, RiskLevel::Medium);
        assert_eq!(capability.input_schema, schema);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn nonmatching_authority_is_refused_before_dispatch_with_empty_evidence() {
    let registry = load().await;
    let failure = registry
        .invoke(
            authorized(
                "skylight.private.account.read",
                constraints("not-skylight.invalid", MAX_REQUEST_BYTES),
            ),
            None,
        )
        .await
        .expect_err("a grant for another authority must be denied");
    assert!(matches!(
        failure.error.as_ref(),
        BrokerHostError::HostCallRejected {
            reason: "denied",
            ..
        }
    ));
    assert!(
        failure.http_calls.is_empty(),
        "pre-dispatch authority refusal must have no HTTP evidence"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn undersized_request_grant_is_refused_before_dispatch_with_empty_evidence() {
    let registry = load().await;
    let failure = registry
        .invoke(
            authorized(
                "skylight.private.frames.list",
                constraints("app.ourskylight.com", 1),
            ),
            None,
        )
        .await
        .expect_err("a one-byte request grant must reject the fixed request");
    assert!(
        matches!(
            failure.error.as_ref(),
            BrokerHostError::HostCallRejected {
                reason: "byte-limit",
                ..
            }
        ),
        "unexpected undersized-request failure: {:#?}",
        failure.error
    );
    assert!(
        failure.http_calls.is_empty(),
        "pre-dispatch request-budget refusal must have no HTTP evidence"
    );
}

#[test]
#[serial_test::serial]
fn committed_broker_limits_are_exact() {
    let limits = host_limits();
    assert_eq!(limits.max_memory_bytes, 32 * 1024 * 1024);
    assert_eq!(limits.fuel, 128_000_000);
    assert_eq!(limits.max_input_bytes, 4_096);
    assert_eq!(limits.max_output_bytes, 32_768);
    assert_eq!(limits.max_http_requests, 1);
    assert_eq!(limits.max_http_request_bytes, 4_096);
    assert_eq!(limits.max_http_response_bytes, 262_144);
    assert_eq!(limits.max_timeout, Duration::from_secs(10));
}

#[tokio::test(flavor = "multi_thread")]
async fn household_reads_have_independent_bounded_get_authority_and_destination_bound_credentials()
{
    let registry = load().await;
    let manifest = registry.manifests().next().unwrap();
    let cases = household_cases::cases();
    assert_eq!(manifest.capabilities.len() - 2, cases.len());
    // Shared cases preserve the manifest's append-only household capability order.
    for (declared, case) in manifest.capabilities[2..].iter().zip(cases) {
        assert_eq!(declared.id.as_str(), case.capability);
        assert_eq!(declared.effect, EffectKind::ReadOnly);
        assert_eq!(declared.risk, RiskLevel::Medium);
        assert_eq!(declared.input_schema["additionalProperties"], false);
        // A fully populated fixture includes optional keys; its size is not the required count.
        let (required, optional): (&[&str], &[&str]) = match case.capability {
            "skylight.private.categories.list" | "skylight.private.lists.list" => {
                (&["frameId"], &[])
            }
            "skylight.private.lists.read" | "skylight.private.list.items.list" => {
                (&["frameId", "listId"], &[])
            }
            "skylight.private.calendar.events.list" => {
                (&["frameId", "dateMin", "dateMax", "timezone"], &["include"])
            }
            "skylight.private.tasks.list" => (
                &["frameId", "after", "before"],
                &["includeLate", "includeUpForGrabs", "filter"],
            ),
            _ => panic!("unrecognized household capability"),
        };
        assert_eq!(declared.input_schema["required"], json!(required));
        let allowed: std::collections::BTreeSet<_> =
            required.iter().chain(optional).copied().collect();
        assert_eq!(
            declared.input_schema["properties"]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>(),
            allowed
        );
        assert_eq!(
            case.input
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>(),
            allowed
        );
        // The shared cases also pin component paths, argv and projections in component_host.
        assert!(
            !case.path.is_empty()
                && !case.key.is_empty()
                && !case.argv.is_empty()
                && case.body.is_object()
        );
        for denied_host in [
            "not-skylight.invalid",
            "app.ourskylight.com.evil.invalid",
            "app.ourskylight.com:444",
        ] {
            let failure = registry
                .invoke(
                    authorized_input(
                        case.capability,
                        case.input.clone(),
                        constraints(denied_host, MAX_REQUEST_BYTES),
                    ),
                    None,
                )
                .await
                .unwrap_err();
            assert!(matches!(
                failure.error.as_ref(),
                BrokerHostError::HostCallRejected {
                    reason: "denied",
                    ..
                }
            ));
            assert!(failure.http_calls.is_empty());
        }
        for (kind, expected) in [("method", "denied"), ("bytes", "byte-limit")] {
            let mut grant = constraints("app.ourskylight.com", MAX_REQUEST_BYTES);
            let http = grant.http.as_mut().unwrap();
            assert_eq!(http.max_requests, 1);
            assert!(!http.allow_plaintext_loopback);
            match kind {
                "method" => http.allowed_methods = vec!["POST".to_owned()],
                "bytes" => http.max_request_bytes = 1,
                _ => unreachable!("fixed test cases"),
            }
            let failure = registry
                .invoke(
                    authorized_input(case.capability, case.input.clone(), grant),
                    None,
                )
                .await
                .unwrap_err();
            assert!(
                matches!(failure.error.as_ref(), BrokerHostError::HostCallRejected { reason, .. } if *reason == expected),
                "{}: {:?}",
                kind,
                failure.error
            );
            assert!(failure.http_calls.is_empty());
        }
        let credential = dekopon_broker_host::BoundCredential::bearer(
            "Bearer",
            Redacted::new("synthetic-credential-sentinel".to_owned()),
            vec!["not-skylight.invalid".to_owned()],
        )
        .unwrap();
        let failure = registry
            .invoke(
                authorized_input(
                    case.capability,
                    case.input.clone(),
                    constraints("app.ourskylight.com", MAX_REQUEST_BYTES),
                ),
                Some(credential),
            )
            .await
            .unwrap_err();
        assert!(matches!(
            failure.error.as_ref(),
            BrokerHostError::HostCallRejected {
                reason: "denied",
                ..
            }
        ));
        assert_eq!(failure.http_calls.len(), 1);
        assert!(!failure.http_calls[0].credential_injected);
        assert_eq!(failure.http_calls[0].status, None);
        assert!(!format!("{failure:?}").contains("synthetic-credential-sentinel"));
    }
}
