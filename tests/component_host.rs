use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use dekopon_provider_host::{HostLimits, ProviderHostError, ProviderRegistry};
use dekopon_provider_sdk::ComponentResponse;
use serde_json::json;
use wasmtime::{
    Config, Engine, ResourceLimiter, Store,
    component::{Component, HasSelf, Linker},
};

const MAX_COMPONENT_BYTES: u64 = 393_216;
const MAX_MEMORY_BYTES: usize = 32 * 1024 * 1024;
const MAX_FUEL: u64 = 128_000_000;
const MAX_INPUT_BYTES: usize = 4_096;
const MAX_REQUEST_BYTES: usize = 4_096;
const MAX_RESPONSE_BYTES: usize = 262_144;
const MAX_OUTPUT_BYTES: usize = 32_768;
const TIMEOUT: Duration = Duration::from_secs(10);

mod bindings {
    wasmtime::component::bindgen!({
        path: "wit",
        world: "provider",
    });
}

use bindings::dekopon::http::client::{Header, HttpError, Request, Response};

fn component_path() -> PathBuf {
    std::env::var_os("DEKOPON_SKYLIGHT_COMPONENT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("dist/provider-skylight-private.wasm")
        })
}

#[derive(Default)]
struct Limits {
    peak_memory_bytes: usize,
}

impl ResourceLimiter for Limits {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        self.peak_memory_bytes = self.peak_memory_bytes.max(current).max(desired);
        Ok(desired <= MAX_MEMORY_BYTES && maximum.is_none_or(|maximum| desired <= maximum))
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(desired <= 10_000 && maximum.is_none_or(|maximum| desired <= maximum))
    }

    fn instances(&self) -> usize {
        100
    }

    fn tables(&self) -> usize {
        100
    }

    fn memories(&self) -> usize {
        100
    }
}

struct State {
    limits: Limits,
    requests: Vec<Request>,
    response: Response,
}

impl bindings::dekopon::http::client::Host for State {
    fn send(&mut self, request: Request) -> Result<Response, HttpError> {
        self.requests.push(request);
        Ok(self.response.clone())
    }
}

fn account_body() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "data": {
            "id": "account-7",
            "attributes": {
                "name": "private-name-sentinel",
                "email": "private-email-sentinel@example.invalid",
                "bearerToken": "private-token-sentinel"
            },
            "relationships": {"sessions": {"data": "private-session-sentinel"}}
        },
        "included": [{"activationCode": "private-activation-sentinel"}]
    }))
    .expect("account response serializes")
}

fn worst_case_frame_body() -> Vec<u8> {
    let data = (0..32)
        .rev()
        .map(|index| {
            let mut id = format!("{index:02}");
            id.push_str(&"\0".repeat(128 - id.len()));
            json!({
                "id": id,
                "attributes": {
                    "name": "\u{0001}".repeat(256),
                    "email": "private-email-sentinel@example.invalid",
                    "bearerToken": "private-token-sentinel"
                },
                "relationships": {"owner": "private-owner-sentinel"}
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_vec(&json!({"data": data})).expect("frame response serializes")
}

fn response(body: Vec<u8>) -> Response {
    assert!(body.len() <= MAX_RESPONSE_BYTES);
    Response {
        status: 200,
        headers: vec![Header {
            name: "content-type".to_owned(),
            value: b"text/private-sentinel".to_vec(),
        }],
        body,
    }
}

fn assert_request(request: &Request, uri: &str) {
    assert_eq!(request.method, "GET");
    assert_eq!(request.uri, uri);
    assert!(request.body.is_empty());
    assert_eq!(request.headers.len(), 2);
    assert_eq!(request.headers[0].name, "accept");
    assert_eq!(request.headers[0].value, b"application/json");
    assert_eq!(request.headers[1].name, "user-agent");
    assert_eq!(
        request.headers[1].value,
        b"dekopon-skylight-private-provider/0.1 (+https://github.com/dekopon-agents/dekopon)"
    );
    for denied in ["authorization", "cookie", "content-type"] {
        assert!(
            !request
                .headers
                .iter()
                .any(|header| header.name.eq_ignore_ascii_case(denied)),
            "guest set denied header {denied}"
        );
    }
    let accounted = request.method.len()
        + request.uri.len()
        + request.body.len()
        + request
            .headers
            .iter()
            .map(|header| header.name.len() + header.value.len())
            .sum::<usize>();
    assert!(accounted <= MAX_REQUEST_BYTES);
}

#[test]
fn immediate_host_refuses_the_sole_privileged_import() {
    let path = component_path();
    let bytes = std::fs::metadata(&path)
        .unwrap_or_else(|error| panic!("build {} first: {error}", path.display()))
        .len();
    assert!(bytes <= MAX_COMPONENT_BYTES);
    let limits = HostLimits {
        max_memory_bytes: MAX_MEMORY_BYTES,
        max_input_bytes: MAX_INPUT_BYTES,
        max_output_bytes: MAX_OUTPUT_BYTES,
        fuel: MAX_FUEL,
        timeout: TIMEOUT,
        ..HostLimits::default()
    };
    let error = ProviderRegistry::load([path], limits)
        .expect_err("the immediate host must not satisfy broker HTTP imports");
    assert!(matches!(error, ProviderHostError::Instantiate { .. }));
}

#[test]
fn in_memory_sole_wit_host_preserves_requests_and_worst_case_projection() {
    let path = component_path();
    let mut config = Config::new();
    config.wasm_component_model(true);
    config.consume_fuel(true);
    let engine = Engine::new(&config).expect("component engine configures");
    let component = Component::from_file(&engine, &path).expect("component compiles");
    let mut linker = Linker::new(&engine);
    bindings::Provider::add_to_linker::<_, HasSelf<_>>(&mut linker, |state: &mut State| state)
        .expect("sole HTTP import links");

    let mut store = Store::new(
        &engine,
        State {
            limits: Limits::default(),
            requests: Vec::new(),
            response: response(account_body()),
        },
    );
    store.limiter(|state| &mut state.limits);
    store.set_fuel(MAX_FUEL).expect("fuel is configured");
    let started = Instant::now();
    let provider = bindings::Provider::instantiate(&mut store, &component, &linker)
        .expect("component instantiates with only the in-memory HTTP host");

    let manifest = provider
        .call_describe(&mut store)
        .expect("describe succeeds");
    let manifest: dekopon_provider_sdk::ProviderManifest =
        serde_json::from_str(&manifest).expect("manifest decodes through the crates.io SDK type");
    assert_eq!(manifest.id.as_str(), "skylight-private");

    let account = provider
        .call_invoke(&mut store, "skylight.private.account.read", "{}")
        .expect("account invocation succeeds");
    assert!(account.len() < MAX_OUTPUT_BYTES);
    assert_eq!(
        serde_json::from_str::<ComponentResponse>(&account).expect("SDK account envelope decodes"),
        ComponentResponse::Succeeded {
            output: json!({"account": {"id": "account-7"}})
        }
    );
    assert_eq!(store.data().requests.len(), 1);
    assert_request(
        &store.data().requests[0],
        "https://app.ourskylight.com/api/user",
    );

    let frame_body = worst_case_frame_body();
    let frame_body_bytes = frame_body.len();
    store.data_mut().response = response(frame_body);
    let frames = provider
        .call_invoke(&mut store, "skylight.private.frames.list", "{}")
        .expect("worst-case frame invocation succeeds");
    assert!(frames.len() < MAX_OUTPUT_BYTES);
    let frames_response: ComponentResponse =
        serde_json::from_str(&frames).expect("actual SDK frame envelope decodes");
    let ComponentResponse::Succeeded { output } = frames_response else {
        panic!("worst-case frame projection unexpectedly failed");
    };
    let projected = output["frames"].as_array().expect("frames are projected");
    assert!(!projected.is_empty());
    assert!(projected.len() < 32, "byte ceiling must omit whole records");
    assert_eq!(output["truncated"], true);
    assert_eq!(
        &projected[0]["id"].as_str().expect("ID is a string")[..2],
        "00"
    );
    let encoded = serde_json::to_vec(&ComponentResponse::Succeeded {
        output: output.clone(),
    })
    .expect("SDK envelope reserializes");
    assert_eq!(encoded, frames.as_bytes());
    assert!(encoded.len() < MAX_OUTPUT_BYTES);
    let rendered = String::from_utf8(encoded).expect("SDK JSON is UTF-8");
    for sentinel in [
        "private-email-sentinel",
        "private-token-sentinel",
        "private-owner-sentinel",
    ] {
        assert!(!rendered.contains(sentinel));
    }

    assert_eq!(store.data().requests.len(), 2);
    assert_request(
        &store.data().requests[1],
        "https://app.ourskylight.com/api/frames",
    );
    assert_eq!("{}".len(), 2);
    assert!(started.elapsed() < TIMEOUT);
    let fuel_remaining = store.get_fuel().expect("fuel remains readable");
    let fuel_consumed = MAX_FUEL - fuel_remaining;
    assert!(fuel_consumed > 0 && fuel_consumed < MAX_FUEL);
    assert!(store.data().limits.peak_memory_bytes < MAX_MEMORY_BYTES);
    eprintln!(
        "measured component host: bytes={} response={} envelope={} records={} peak-memory={} fuel={} elapsed-ms={}",
        std::fs::metadata(path).expect("component metadata").len(),
        frame_body_bytes,
        frames.len(),
        projected.len(),
        store.data().limits.peak_memory_bytes,
        fuel_consumed,
        started.elapsed().as_millis(),
    );
}

#[test]
fn committed_component_limits_are_exact() {
    assert_eq!(MAX_COMPONENT_BYTES, 393_216);
    assert_eq!(MAX_MEMORY_BYTES, 32 * 1024 * 1024);
    assert_eq!(MAX_FUEL, 128_000_000);
    assert_eq!(MAX_INPUT_BYTES, 4_096);
    assert_eq!(MAX_REQUEST_BYTES, 4_096);
    assert_eq!(MAX_RESPONSE_BYTES, 262_144);
    assert_eq!(MAX_OUTPUT_BYTES, 32_768);
    assert_eq!(TIMEOUT, Duration::from_secs(10));
}
