use std::{
    io::Write,
    path::PathBuf,
    time::{Duration, Instant},
};

use dekopon_provider_sdk::{CommandRunOutcome, ComponentFailure, ComponentResponse};
use serde_json::json;
use wasmtime::{
    Config, Engine, ResourceLimiter, Store,
    component::{Component, HasSelf, Linker},
};

mod household_cases;

const MAX_COMPONENT_BYTES: u64 = 393_216;
const MAX_MEMORY_BYTES: usize = 32 * 1024 * 1024;
const MAX_FUEL: u64 = 128_000_000;
const MAX_INPUT_BYTES: usize = 4_096;
const MAX_REQUEST_BYTES: usize = 4_096;
const MAX_RESPONSE_BYTES: usize = 262_144;
const MAX_OUTPUT_BYTES: usize = 32_768;
const TIMEOUT: Duration = Duration::from_secs(10);
const NEAR_LIMIT_FRAME_COUNT: usize = 20_000;

mod bindings {
    wasmtime::component::bindgen!({
        path: "wit",
        world: "provider",
    });
}

use bindings::dekopon::http::client::{Header, HttpError, Request, Response};

fn component_path() -> PathBuf {
    PathBuf::from(
        std::env::var_os("DEKOPON_PROVIDER_COMPONENT")
            .expect("DEKOPON_PROVIDER_COMPONENT must point at the built component"),
    )
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

#[derive(Clone, Copy, Debug)]
enum RecordOrder {
    Permuted,
    Descending,
}

/// Produces a 260,010-byte review probe without a fixture or random dependency. Multiplication by
/// 7,919 permutes all 20,000 indices; descending order replaces every retained record. Each selected
/// scalar is exactly three UTF-8 bytes.
fn near_limit_frame_body(malformed_last: bool, order: RecordOrder) -> Vec<u8> {
    let mut body = Vec::with_capacity(MAX_RESPONSE_BYTES);
    body.extend_from_slice(br#"{"data":["#);
    for position in 0..NEAR_LIMIT_FRAME_COUNT {
        if position != 0 {
            body.push(b',');
        }
        let index = match order {
            RecordOrder::Permuted => (position * 7_919 + 1_237) % NEAR_LIMIT_FRAME_COUNT,
            RecordOrder::Descending => NEAR_LIMIT_FRAME_COUNT - 1 - position,
        };
        let id = char::from_u32(0x0800 + index as u32).expect("fixture scalar is valid");
        if malformed_last && position + 1 == NEAR_LIMIT_FRAME_COUNT {
            write!(
                &mut body,
                r#"{{"id":"{id}","attributes":{{"name":"ok","name":null}}}}"#
            )
            .expect("writing to a byte vector succeeds");
        } else {
            write!(&mut body, r#"{{"id":"{id}"}}"#).expect("writing to a byte vector succeeds");
        }
    }
    body.extend_from_slice(b"]}");
    if malformed_last {
        assert_eq!(body.len(), 260_049);
    } else {
        assert_eq!(body.len(), 260_010);
    }
    assert!(body.len() <= MAX_RESPONSE_BYTES);
    body
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

fn instantiate(response: Response) -> (Store<State>, bindings::Provider) {
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
            response,
        },
    );
    store.limiter(|state| &mut state.limits);
    store.set_fuel(MAX_FUEL).expect("fuel is configured");
    let provider = bindings::Provider::instantiate(&mut store, &component, &linker)
        .expect("component instantiates with only the in-memory HTTP host");
    (store, provider)
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

/// A host that links nothing must not be able to instantiate this component.
///
/// The property is the component's, not any one host's: `dekopon:http/client@1.0.0` is privileged
/// and is satisfied only by a host that deliberately links it. This is asserted against an empty
/// Wasmtime linker rather than a named import-free host crate so it keeps holding as the Dekopon
/// tree rearranges its hosts.
#[test]
fn immediate_host_refuses_the_sole_privileged_import() {
    let path = component_path();
    let bytes = std::fs::metadata(&path)
        .unwrap_or_else(|error| panic!("build {} first: {error}", path.display()))
        .len();
    assert!(bytes <= MAX_COMPONENT_BYTES);
    let mut config = Config::new();
    config.wasm_component_model(true);
    config.consume_fuel(true);
    let engine = Engine::new(&config).expect("component engine configures");
    let component = Component::from_file(&engine, &path).expect("component compiles");
    let linker: Linker<State> = Linker::new(&engine);
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
    let error = match bindings::Provider::instantiate(&mut store, &component, &linker) {
        Ok(_) => panic!("an import-free host must not satisfy broker HTTP imports"),
        Err(error) => error,
    };
    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("dekopon:http/client"),
        "refusal must name the unsatisfied privileged import: {rendered}"
    );
}

#[test]
#[serial_test::serial]
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
fn component_boundary_pins_unknown_precedence_and_malformed_json() {
    let (mut store, provider) = instantiate(response(account_body()));
    let cases = [
        (
            "skylight.private.unknown",
            "{not-json",
            "unknown-capability",
            "unsupported Skylight private capability",
        ),
        (
            "not a capability",
            "[also-not-json",
            "unknown-capability",
            "unsupported Skylight private capability",
        ),
        (
            "skylight.private.account.read",
            "{not-json",
            "invalid-input",
            "input must be exactly an empty object",
        ),
        (
            "skylight.private.frames.list",
            r#"{"endpoint":"caller-controlled.invalid"}"#,
            "invalid-input",
            "input must be exactly an empty object",
        ),
    ];

    for (capability, input, code, message) in cases {
        let encoded = provider
            .call_invoke(&mut store, capability, input)
            .expect("boundary refusal returns rather than traps");
        assert_eq!(
            serde_json::from_str::<ComponentResponse>(&encoded)
                .expect("boundary response is an SDK envelope"),
            ComponentResponse::Failed {
                error: ComponentFailure {
                    code: code.to_owned(),
                    message: message.to_owned(),
                }
            }
        );
    }
    assert!(
        store.data().requests.is_empty(),
        "wire-level capability and input failures must precede HTTP"
    );
}

#[test]
#[serial_test::serial]
fn near_limit_random_order_projects_within_committed_fuel() {
    let body = near_limit_frame_body(false, RecordOrder::Permuted);
    let response_bytes = body.len();
    let started = Instant::now();
    let (mut store, provider) = instantiate(response(body));
    let encoded = provider
        .call_invoke(&mut store, "skylight.private.frames.list", "{}")
        .expect("near-limit valid response must not trap");
    let ComponentResponse::Succeeded { output } =
        serde_json::from_str::<ComponentResponse>(&encoded).expect("SDK envelope decodes")
    else {
        panic!("near-limit valid response unexpectedly failed");
    };

    let frames = output["frames"].as_array().expect("frames are projected");
    assert_eq!(frames.len(), 32);
    for (index, frame) in frames.iter().enumerate() {
        let expected = char::from_u32(0x0800 + index as u32)
            .expect("expected scalar is valid")
            .to_string();
        assert_eq!(frame["id"], expected);
        assert_eq!(frame["nameTruncated"], false);
    }
    assert_eq!(output["truncated"], true);
    assert!(encoded.len() < MAX_OUTPUT_BYTES);
    assert_eq!(store.data().requests.len(), 1);
    assert_request(
        &store.data().requests[0],
        "https://app.ourskylight.com/api/frames",
    );
    assert!(started.elapsed() < TIMEOUT);
    let fuel_consumed = MAX_FUEL - store.get_fuel().expect("fuel remains readable");
    assert!(fuel_consumed > 0 && fuel_consumed < MAX_FUEL);
    assert!(store.data().limits.peak_memory_bytes < MAX_MEMORY_BYTES);
    eprintln!(
        "measured near-limit valid response: response={} envelope={} records={} peak-memory={} fuel={} elapsed-ms={}",
        response_bytes,
        encoded.len(),
        frames.len(),
        store.data().limits.peak_memory_bytes,
        fuel_consumed,
        started.elapsed().as_millis(),
    );
}

#[test]
#[serial_test::serial]
fn near_limit_malformed_last_record_fails_closed_within_committed_fuel() {
    let body = near_limit_frame_body(true, RecordOrder::Permuted);
    let response_bytes = body.len();
    let started = Instant::now();
    let (mut store, provider) = instantiate(response(body));
    let encoded = provider
        .call_invoke(&mut store, "skylight.private.frames.list", "{}")
        .expect("near-limit malformed response must return rather than trap");
    assert_eq!(
        serde_json::from_str::<ComponentResponse>(&encoded).expect("SDK envelope decodes"),
        ComponentResponse::Failed {
            error: ComponentFailure {
                code: "invalid-response".to_owned(),
                message: "the private API returned an invalid response".to_owned(),
            }
        }
    );
    assert_eq!(store.data().requests.len(), 1);
    assert_request(
        &store.data().requests[0],
        "https://app.ourskylight.com/api/frames",
    );
    assert!(started.elapsed() < TIMEOUT);
    let fuel_consumed = MAX_FUEL - store.get_fuel().expect("fuel remains readable");
    assert!(fuel_consumed > 0 && fuel_consumed < MAX_FUEL);
    assert!(store.data().limits.peak_memory_bytes < MAX_MEMORY_BYTES);
    eprintln!(
        "measured near-limit malformed-last response: response={} envelope={} peak-memory={} fuel={} elapsed-ms={}",
        response_bytes,
        encoded.len(),
        store.data().limits.peak_memory_bytes,
        fuel_consumed,
        started.elapsed().as_millis(),
    );
}

/// `run-command` answers from the guest alone: a proposal, the help page, or the usage error, and
/// never a call through the HTTP import, whatever argv or piped value arrives.
#[test]
fn run_command_proposes_or_renders_without_touching_the_http_import() {
    let (mut store, provider) = instantiate(response(account_body()));
    let mut run = |words: &[&str]| -> CommandRunOutcome {
        let argv = words
            .iter()
            .map(|word| (*word).to_owned())
            .collect::<Vec<_>>();
        let encoded = provider
            .call_run_command(&mut store, &argv, Some("piped-sentinel"))
            .expect("run-command returns rather than traps");
        serde_json::from_str(&encoded).expect("run-command result is an SDK outcome")
    };

    for (verb, capability) in [
        ("account", "skylight.private.account.read"),
        ("frames", "skylight.private.frames.list"),
    ] {
        assert_eq!(
            run(&[verb]),
            CommandRunOutcome::Proposed {
                capability: capability.parse().expect("valid capability fixture"),
                input: json!({}),
                secret_use: None,
            }
        );
    }

    let CommandRunOutcome::Rendered {
        stdout,
        stderr,
        status,
    } = run(&["--help"])
    else {
        panic!("help must render");
    };
    assert_eq!(status, 0);
    assert!(
        stdout.starts_with("Unsupported private Skylight"),
        "{stdout}"
    );
    assert!(stderr.is_empty(), "{stderr}");

    let CommandRunOutcome::Rendered {
        stdout,
        stderr,
        status,
    } = run(&["frames", "caller-controlled-sentinel"])
    else {
        panic!("a usage error must render");
    };
    assert_eq!(status, 2);
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.starts_with("error: "), "{stderr}");
    for sentinel in ["caller-controlled-sentinel", "piped-sentinel"] {
        assert!(!stderr.contains(sentinel), "usage error echoed {sentinel}");
    }

    assert!(
        store.data().requests.is_empty(),
        "run-command must never reach the HTTP import"
    );
}

#[test]
#[serial_test::serial]
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

#[test]
fn household_capabilities_cross_component_boundary_without_ambient_authority() {
    for case in household_cases::cases() {
        let (mut store, provider) = instantiate(response(serde_json::to_vec(&case.body).unwrap()));
        let argv: Vec<_> = case.argv.iter().map(|s| (*s).to_owned()).collect();
        let proposed = provider
            .call_run_command(&mut store, &argv, Some("secret-sentinel"))
            .unwrap();
        assert_eq!(
            serde_json::from_str::<CommandRunOutcome>(&proposed).unwrap(),
            CommandRunOutcome::Proposed {
                capability: case.capability.parse().unwrap(),
                input: case.input.clone(),
                secret_use: None
            }
        );
        assert!(store.data().requests.is_empty());
        let encoded = provider
            .call_invoke(&mut store, case.capability, &case.input.to_string())
            .unwrap();
        let ComponentResponse::Succeeded { output } = serde_json::from_str(&encoded).unwrap()
        else {
            panic!("household read failed");
        };
        assert_eq!(output["upstreamCompleteness"], "unknown");
        assert_eq!(output["coverage"], "bounded-response");
        let record = if case.key == "list" {
            &output[case.key]
        } else {
            &output[case.key][0]
        };
        assert_eq!(record["id"], "sample");
        if case.key == "events" {
            assert_eq!(record["startsAt"], "2028-03-10T23:00:00-05:00");
            assert_eq!(record["endsAt"], "2028-03-14T01:00:00-04:00");
            assert_eq!(record["allDay"], false);
        }
        if case.key == "items" {
            assert!(record["status"].is_null());
        }
        assert!(encoded.len() < MAX_OUTPUT_BYTES);
        assert_eq!(store.data().requests.len(), 1);
        assert_request(
            &store.data().requests[0],
            &format!(
                "https://app.ourskylight.com/api/frames/frame-test{}",
                case.path
            ),
        );
        assert!(store.data().limits.peak_memory_bytes < MAX_MEMORY_BYTES);
        assert!(store.get_fuel().unwrap() > 0);
        for input in [
            r#"{"frameId":"x","frameId":"y"}"#,
            r#"{"frameId":null,"frameId":"y"}"#,
            r#"{"frameId":"x","url":"secret-sentinel"}"#,
            "{broken",
        ] {
            let encoded = provider
                .call_invoke(&mut store, case.capability, input)
                .unwrap();
            assert!(
                matches!(serde_json::from_str::<ComponentResponse>(&encoded).unwrap(), ComponentResponse::Failed {error} if error.code == "invalid-input" && error.message == "input must match the bounded Skylight read schema")
            );
        }
        assert_eq!(store.data().requests.len(), 1);
    }
}

#[test]
#[serial_test::serial]
fn household_near_response_limit_validates_tail_within_unchanged_resource_limits() {
    assert_household_near_response_limit(RecordOrder::Permuted);
}

#[test]
#[serial_test::serial]
fn household_descending_near_response_limit_validates_tail_within_unchanged_resource_limits() {
    assert_household_near_response_limit(RecordOrder::Descending);
}

fn assert_household_near_response_limit(order: RecordOrder) {
    for malformed in [false, true] {
        // 20,000 minimal records under 256 KiB exercise streaming retention, not a household fixture.
        let mut body = near_limit_frame_body(false, order);
        if malformed {
            body.truncate(body.len() - 2);
            body.extend_from_slice(
                br#",{"id":"tail","attributes":{"all_day":null,"all_day":true}}]}"#,
            );
        }
        let bytes = body.len();
        let started = Instant::now();
        let (mut store, provider) = instantiate(response(body));
        let encoded = provider.call_invoke(&mut store,"skylight.private.calendar.events.list",r#"{"frameId":"frame-test","dateMin":"2028-03-11","dateMax":"2028-03-13","timezone":"America/New_York"}"#).expect("bounded response must not trap");
        match serde_json::from_str::<ComponentResponse>(&encoded).unwrap() {
            ComponentResponse::Succeeded { output } if !malformed => {
                let events = output["events"].as_array().unwrap();
                assert_eq!(events.len(), 64);
                for (index, event) in events.iter().enumerate() {
                    assert_eq!(
                        event["id"],
                        char::from_u32(0x0800 + index as u32).unwrap().to_string()
                    );
                    for field in ["summary", "startsAt", "endsAt", "allDay"] {
                        assert!(event[field].is_null());
                    }
                    assert_eq!(event["textTruncated"], false);
                }
                assert_eq!(output["coverage"], "bounded-response");
                assert_eq!(output["truncated"], true);
                assert_eq!(output["upstreamCompleteness"], "unknown");
            }
            ComponentResponse::Failed { error } if malformed => {
                assert_eq!(error.code, "invalid-response");
                assert_eq!(
                    error.message,
                    "the private API returned an invalid response"
                );
            }
            _ => panic!("unexpected projection result"),
        }
        assert!(encoded.len() < MAX_OUTPUT_BYTES);
        assert!(store.data().limits.peak_memory_bytes < MAX_MEMORY_BYTES);
        assert!(started.elapsed() < TIMEOUT);
        eprintln!(
            "household near-limit: order={order:?} malformed={} bytes={} envelope={} memory={} fuel={} elapsed-ms={}",
            malformed,
            bytes,
            encoded.len(),
            store.data().limits.peak_memory_bytes,
            MAX_FUEL - store.get_fuel().unwrap(),
            started.elapsed().as_millis()
        );
    }
}

#[test]
#[serial_test::serial]
fn calendar_escaped_text_output_budget_stays_within_committed_limits() {
    let data: Vec<_> = (0..40)
        .map(|i| json!({"id":format!("{i:03}{}", "\0".repeat(125)), "attributes":{"summary":"\0".repeat(256),"starts_at":"\0".repeat(256),"ends_at":"\0".repeat(256)}}))
        .collect();
    let body = serde_json::to_vec(&json!({"data":data})).unwrap();
    let response_bytes = body.len();
    let started = Instant::now();
    let (mut store, provider) = instantiate(response(body));
    let encoded = provider.call_invoke(&mut store, "skylight.private.calendar.events.list", r#"{"frameId":"frame-test","dateMin":"2028-03-11","dateMax":"2028-03-13","timezone":"America/New_York"}"#).expect("escaped output must not trap");
    let ComponentResponse::Succeeded { output } = serde_json::from_str(&encoded).unwrap() else {
        panic!("budget projection failed");
    };
    let records = output["events"].as_array().unwrap().len();
    assert!(records > 0 && records < 40);
    assert_eq!(output["truncated"], true);
    assert_eq!(output["upstreamCompleteness"], "unknown");
    assert!(encoded.len() < MAX_OUTPUT_BYTES);
    assert!(store.data().limits.peak_memory_bytes < MAX_MEMORY_BYTES);
    assert!(started.elapsed() < TIMEOUT);
    assert_eq!(store.data().requests.len(), 1);
    eprintln!(
        "calendar escaped-text budget: response={} envelope={} records={} memory={} fuel={} elapsed-ms={}",
        response_bytes,
        encoded.len(),
        records,
        store.data().limits.peak_memory_bytes,
        MAX_FUEL - store.get_fuel().unwrap(),
        started.elapsed().as_millis()
    );
}
