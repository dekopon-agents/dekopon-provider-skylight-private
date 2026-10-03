//! Broker-authorized real-component refusal tests. No call may reach a private host.
use std::{path::PathBuf, time::Duration};

use dekopon_broker_host::{
    BoundCredential, BrokerHostError, BrokerHostLimits, BrokerProviderRegistry, asset::AssetInputs,
};
use dekopon_capability::{
    AuthorizedInvocation, ExecutionConstraints, HttpConstraints, ProposedInvocation,
    broker::AuthorizationGate,
};
use dekopon_core::{Actor, AgentId, InvocationId, PrincipalId, Redacted, TraceId};
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
            "skylight-test-policy".to_owned(),
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
    ];
    assert_eq!(
        registry.manifests().next().unwrap().capabilities.len(),
        cases.len()
    );
    for (capability, input) in cases {
        for (host, method, bytes, reason) in [
            ("not-skylight.invalid", "GET", 4096, "denied"),
            ("app.ourskylight.com.evil.invalid", "GET", 4096, "denied"),
            ("app.ourskylight.com:444", "GET", 4096, "denied"),
            ("app.ourskylight.com", "POST", 4096, "denied"),
            ("app.ourskylight.com", "GET", 1, "byte-limit"),
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
                matches!(failure.error.as_ref(), BrokerHostError::HostCallRejected { reason: actual, .. } if *actual == reason),
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
        let failure = registry
            .invoke(
                authorized(capability, input, grant("app.ourskylight.com", "GET", 4096)),
                Some(credential),
                AssetInputs::default(),
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
