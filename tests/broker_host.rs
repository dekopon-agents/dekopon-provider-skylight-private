use std::{path::PathBuf, time::Duration};

use dekopon_broker_host::{BrokerHostError, BrokerHostLimits, BrokerProviderRegistry};
use dekopon_capability::{
    AuthorizedInvocation, EffectKind, ExecutionConstraints, HttpConstraints, Idempotency,
    ProposedInvocation, broker::AuthorizationGate,
};
use dekopon_core::{Actor, AgentId, InvocationId, PrincipalId, RiskLevel, TraceId};
use dekopon_provider_sdk::ProviderApiVersion;
use serde_json::json;

const MAX_COMPONENT_BYTES: u64 = 393_216;
const MAX_MEMORY_BYTES: usize = 32 * 1024 * 1024;
const MAX_FUEL: u64 = 128_000_000;
const MAX_INPUT_BYTES: usize = 4_096;
const MAX_REQUEST_BYTES: u64 = 4_096;
const MAX_RESPONSE_BYTES: u64 = 262_144;
const MAX_OUTPUT_BYTES: u64 = 32_768;
const TIMEOUT_MS: u64 = 10_000;

fn component_path() -> PathBuf {
    std::env::var_os("DEKOPON_SKYLIGHT_COMPONENT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("dist/provider-skylight-private.wasm")
        })
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
    }
}

fn authorized(capability: &str, constraints: ExecutionConstraints) -> AuthorizedInvocation {
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
        "skylight-test-trace"
            .parse::<TraceId>()
            .expect("valid trace fixture"),
        json!({}),
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
    assert!(manifest.command_words.is_empty());
    assert_eq!(manifest.capabilities.len(), 2);

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
        assert_eq!(capability.idempotency, Idempotency::Idempotent);
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
