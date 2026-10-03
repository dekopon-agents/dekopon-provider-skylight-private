//! Real stdio component, broker-host and native boundaries; synthetic account data only.
use std::{
    io::Write as _,
    path::PathBuf,
    time::{Duration, Instant},
};

use dekopon_provider_sdk::{
    CommandRunOutcome,
    provider::{self, Response},
};
use dekopon_provider_sdk_testkit::{Harness, HarnessError, HttpScript, Native, conformance};
use dekopon_skylight_private_provider::SkylightPrivate;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const ACCOUNT: &str = "skylight-private.account.read";
const FRAMES: &str = "skylight-private.frames.list";
const CATEGORIES: &str = "skylight-private.categories.list";
const HOST: &str = "app.ourskylight.com";
fn component() -> PathBuf {
    std::env::var_os("DEKOPON_PROVIDER_COMPONENT")
        .expect("build the real component and set DEKOPON_PROVIDER_COMPONENT")
        .into()
}
fn response(body: Value) -> Response {
    Response {
        status: 200,
        headers: vec![],
        body: serde_json::to_vec(&body).unwrap(),
    }
}
fn account_fixture() -> Value {
    json!({"data":{"id":"account-synthetic","attributes":{"bearerToken":"never-emit-sentinel","email":"private-sentinel"}}})
}

#[test]
fn component_conforms_and_declares_only_broker_imports() -> TestResult {
    conformance::<SkylightPrivate>(component())?;
    let manifest = provider::manifest::<SkylightPrivate>()?;
    assert_eq!(manifest.id.as_str(), "skylight-private");
    assert_eq!(manifest.capabilities.len(), 8);
    let expected_ids = [
        ACCOUNT,
        FRAMES,
        CATEGORIES,
        "skylight-private.calendar.events.list",
        "skylight-private.lists.list",
        "skylight-private.lists.read",
        "skylight-private.list.items.list",
        "skylight-private.tasks.list",
    ];
    assert_eq!(
        manifest
            .capabilities
            .iter()
            .map(|cap| cap.id.as_str())
            .collect::<Vec<_>>(),
        expected_ids
    );
    for cap in &manifest.capabilities {
        assert!(cap.id.as_str().starts_with("skylight-private."));
        assert_eq!(cap.effect, dekopon_provider_sdk::EffectKind::ReadOnly);
        assert_eq!(cap.risk, dekopon_provider_sdk::RiskLevel::Medium);
        assert_eq!(cap.input_schema["additionalProperties"], false);
    }
    let frame = &manifest.capabilities[2].input_schema["properties"]["frameId"];
    assert_eq!(frame["minLength"], 1);
    assert_eq!(frame["maxLength"], 128);
    assert_eq!(frame["pattern"], "^[A-Za-z0-9_-]+$");
    let events = &manifest.capabilities[3].input_schema;
    assert_eq!(
        events["required"],
        json!(["frameId", "dateMin", "dateMax", "timezone"])
    );
    assert_eq!(events["properties"]["dateMin"]["format"], "date");
    assert_eq!(
        events["properties"]["include"]["enum"],
        json!(["categories,calendar_account,event_notification_setting"])
    );
    let tasks = &manifest.capabilities[7].input_schema;
    assert_eq!(tasks["required"], json!(["frameId", "after", "before"]));
    assert_eq!(tasks["properties"]["includeLate"]["type"], "boolean");
    assert_eq!(tasks["properties"]["includeLate"]["default"], false);
    assert_eq!(
        tasks["properties"]["filter"]["enum"],
        json!(["linked_to_profile"])
    );
    Ok(())
}

#[test]
fn cli_proposes_exact_ids_without_reading_stdin_or_granting_http() {
    let manifest = provider::manifest::<SkylightPrivate>().unwrap();
    for (argv, id, input) in [
        (vec!["account"], ACCOUNT, json!({})),
        (vec!["frames"], FRAMES, json!({})),
        (
            vec!["categories", "--frame", "frame-a"],
            CATEGORIES,
            json!({"frameId":"frame-a"}),
        ),
        (
            vec![
                "tasks",
                "--frame",
                "frame-a",
                "--after",
                "2028-03-11",
                "--before",
                "2028-03-11",
            ],
            "skylight-private.tasks.list",
            json!({"frameId":"frame-a","after":"2028-03-11","before":"2028-03-11"}),
        ),
    ] {
        for piped in [false, true] {
            let CommandRunOutcome::Proposed {
                capability,
                input: proposed,
                secret_use,
            } = provider::command::<SkylightPrivate>(
                &argv
                    .iter()
                    .map(|word| (*word).to_owned())
                    .collect::<Vec<_>>(),
                piped,
            )
            else {
                panic!("valid arguments must propose");
            };
            assert_eq!(capability.as_str(), id);
            assert_eq!(proposed, input);
            assert!(secret_use.is_none());
            assert!(manifest.capabilities.iter().any(|cap| cap.id == capability));
        }
    }
    for argv in [
        vec!["account", "--frame", "frame-a"],
        vec!["categories", "--frame", "../outside"],
        vec![
            "events",
            "--frame",
            "frame-a",
            "--from",
            "2028-01-01",
            "--to",
            "2028-01-02",
        ],
    ] {
        let outcome = provider::command::<SkylightPrivate>(
            &argv
                .iter()
                .map(|word| (*word).to_owned())
                .collect::<Vec<_>>(),
            true,
        );
        assert!(!matches!(outcome, CommandRunOutcome::Proposed { .. }));
        assert!(!format!("{outcome:?}").contains("../outside"));
    }
    for argv in [vec!["--help"], vec!["help"], vec!["account", "--help"]] {
        assert!(matches!(
            provider::command::<SkylightPrivate>(
                &argv
                    .iter()
                    .map(|word| (*word).to_owned())
                    .collect::<Vec<_>>(),
                true
            ),
            CommandRunOutcome::Rendered { status: 0, .. }
        ));
    }
}

#[test]
fn real_account_matches_native_and_excludes_private_response_fields() -> TestResult {
    let fixture = HttpScript::new(HOST, "GET", response(account_fixture()));
    let native = Native::<SkylightPrivate>::new().http(fixture);
    let expected = native.call(ACCOUNT, "{}");
    assert_eq!(expected.status, 0);
    assert_eq!(native.requests().len(), 1);
    let request = &native.requests()[0];
    assert_eq!(request.uri, "https://app.ourskylight.com/api/user");
    assert_eq!(request.method, "GET");
    assert!(request.body.is_empty());
    assert!(
        !request
            .headers
            .iter()
            .any(|h| h.name.eq_ignore_ascii_case("authorization")
                || h.name.eq_ignore_ascii_case("cookie"))
    );
    // Fixed :443 is not the testkit's ephemeral HTTPS port. No real component success is claimed.
    let stdout = String::from_utf8(expected.stdout)?;
    assert_eq!(
        serde_json::from_str::<Value>(&stdout)?,
        json!({"account":{"id":"account-synthetic"}})
    );
    assert!(!stdout.contains("never-emit-sentinel"));
    assert!(!stdout.contains("private-sentinel"));
    Ok(())
}

#[test]
fn all_eight_reads_retain_bounded_native_projections() -> TestResult {
    for (capability, input, body, expected_path, output_key) in [
        (
            FRAMES,
            json!({}),
            json!({"data":[{"id":"frame-b","attributes":{"name":"Room"}},{"id":"frame-a"}]}),
            "/api/frames",
            "frames",
        ),
        (
            CATEGORIES,
            json!({"frameId":"frame-a"}),
            json!({"data":[]}),
            "/api/frames/frame-a/categories",
            "categories",
        ),
        (
            "skylight-private.calendar.events.list",
            json!({"frameId":"frame-a","dateMin":"2028-03-11","dateMax":"2028-03-13","timezone":"America/New_York"}),
            json!({"data":[]}),
            "/api/frames/frame-a/calendar_events?date_min=2028-03-11&date_max=2028-03-13&timezone=America%2FNew_York",
            "events",
        ),
        (
            "skylight-private.lists.list",
            json!({"frameId":"frame-a"}),
            json!({"data":[]}),
            "/api/frames/frame-a/lists",
            "lists",
        ),
        (
            "skylight-private.lists.read",
            json!({"frameId":"frame-a","listId":"list-a"}),
            json!({"data":{"id":"list-a"}}),
            "/api/frames/frame-a/lists/list-a",
            "list",
        ),
        (
            "skylight-private.list.items.list",
            json!({"frameId":"frame-a","listId":"list-a"}),
            json!({"data":[]}),
            "/api/frames/frame-a/lists/list-a/list_items",
            "items",
        ),
        (
            "skylight-private.tasks.list",
            json!({"frameId":"frame-a","after":"2028-03-11","before":"2028-03-11"}),
            json!({"data":[]}),
            "/api/frames/frame-a/chores?after=2028-03-11&before=2028-03-11&include_late=false&include_up_for_grabs=false&filter=linked_to_profile",
            "tasks",
        ),
    ] {
        let fixture = HttpScript::new(HOST, "GET", response(body));
        let native = Native::<SkylightPrivate>::new().http(fixture);
        let output = native.call(capability, &input.to_string());
        assert_eq!(output.status, 0, "{capability}: {}", output.stderr);
        let requests = native.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].uri, format!("https://{HOST}{expected_path}"));
        assert_eq!(requests[0].method, "GET");
        assert!(requests[0].body.is_empty());
        assert!(
            requests[0]
                .headers
                .iter()
                .all(|header| !matches!(header.name.as_str(), "authorization" | "cookie"))
        );
        let value: Value = serde_json::from_slice(&output.stdout)?;
        assert!(value.get(output_key).is_some(), "{capability}: {value}");
        if capability == FRAMES {
            assert_eq!(
                value["frames"],
                json!([{"id":"frame-a","nameTruncated":false},{"id":"frame-b","name":"Room","nameTruncated":false}])
            );
        } else {
            assert_eq!(value["coverage"], "bounded-response");
            assert_eq!(value["upstreamCompleteness"], "unknown");
        }
    }
    Ok(())
}

/// 20,000 JSON resources, 260,010 bytes (below the 256 KiB response limit). No real
/// destination or credential is used; the testkit returns these bytes in process.
fn near_limit_body(malformed: bool, descending: bool, capability: &str) -> Vec<u8> {
    let mut body = Vec::with_capacity(262_144);
    body.extend_from_slice(br#"{"data":["#);
    for position in 0..20_000 {
        if position != 0 {
            body.push(b',');
        }
        let index = if descending {
            19_999 - position
        } else {
            (position * 7_919 + 1_237) % 20_000
        };
        let id = char::from_u32(0x0800 + index as u32).unwrap();
        if malformed && position == 19_999 {
            match capability {
                FRAMES => write!(&mut body, r#"{{"id":"{id}","attributes":{{"name":"ok","name":null}}}}"#).unwrap(),
                "skylight-private.calendar.events.list" => write!(&mut body, r#"{{"id":"{id}","attributes":{{"all_day":null,"all_day":true}}}}"#).unwrap(),
                "skylight-private.tasks.list" => write!(&mut body, r#"{{"id":"{id}","attributes":{{"recurrence_set":["RRULE:FREQ=DAILY",false]}}}}"#).unwrap(),
                _ => unreachable!("bounded fixture capability"),
            }
        } else {
            write!(&mut body, r#"{{"id":"{id}"}}"#).unwrap();
        }
    }
    body.extend_from_slice(b"]}");
    if !malformed {
        assert_eq!(body.len(), 260_010);
    }
    assert!((260_000..=262_144).contains(&body.len()));
    body
}

#[test]
fn native_near_response_limit_validates_every_tail_and_bounds_output_and_deadline() -> TestResult {
    for descending in [false, true] {
        for malformed in [false, true] {
            for (capability, input, key, retained) in [
                (FRAMES, "{}", "frames", 32),
                (
                    "skylight-private.calendar.events.list",
                    r#"{"frameId":"frame-test","dateMin":"2028-03-11","dateMax":"2028-03-13","timezone":"America/New_York"}"#,
                    "events",
                    64,
                ),
                (
                    "skylight-private.tasks.list",
                    r#"{"frameId":"frame-test","after":"2028-03-11","before":"2028-03-11"}"#,
                    "tasks",
                    64,
                ),
            ] {
                let body = near_limit_body(malformed, descending, capability);
                let native = Native::<SkylightPrivate>::new().http(HttpScript::new(
                    HOST,
                    "GET",
                    Response {
                        status: 200,
                        headers: vec![],
                        body: body.clone(),
                    },
                ));
                let started = Instant::now();
                let output = native.call(capability, input);
                assert!(
                    started.elapsed() < Duration::from_secs(10),
                    "{capability}: native deadline"
                );
                assert_eq!(
                    native.requests().len(),
                    1,
                    "{capability}: exactly one fixed GET"
                );
                assert!(
                    output.stdout.len() < 32 * 1024,
                    "{capability}: output bound"
                );
                assert!(
                    body.len() + output.stdout.len() < 32 * 1024 * 1024,
                    "bounded input/output below guest-memory ceiling; native peak is not measured"
                );
                if malformed {
                    assert_eq!(output.status, 1, "{capability}: {}", output.stderr);
                    assert!(output.stdout.is_empty());
                    assert_eq!(
                        output.stderr,
                        "the private API returned an invalid response\n"
                    );
                } else {
                    assert_eq!(output.status, 0, "{capability}: {}", output.stderr);
                    let value: Value = serde_json::from_slice(&output.stdout)?;
                    let records = value[key].as_array().unwrap();
                    assert_eq!(records.len(), retained);
                    for (index, record) in records.iter().enumerate() {
                        assert_eq!(
                            record["id"],
                            char::from_u32(0x0800 + index as u32).unwrap().to_string()
                        );
                    }
                    assert_eq!(value["truncated"], true);
                }
            }
        }
    }
    Ok(())
}

#[test]
fn malformed_inputs_and_unknown_ids_fail_before_any_http() -> TestResult {
    for (capability, input) in [
        (ACCOUNT, json!({"url":"https://untrusted.invalid"})),
        (CATEGORIES, json!({"frameId":"../escape"})),
        (CATEGORIES, json!({"frameId":[]})),
        (
            "skylight-private.tasks.list",
            json!({"frameId":"frame-test","after":"2028-03-11","before":"2028-03-11","includeLate":null}),
        ),
        (
            "skylight-private.calendar.events.list",
            json!({"frameId":"frame-test","dateMin":"2028-03-11","dateMax":"2028-03-13","timezone":"America/New_York","include":null}),
        ),
    ] {
        let actual = Harness::<SkylightPrivate>::get(component()).call(capability, input)?;
        assert_ne!(actual.status, 0);
        assert!(actual.stdout.is_empty());
        assert!(actual.http_calls.is_empty());
        assert!(!actual.stderr.contains("untrusted.invalid"));
        assert!(!actual.stderr.contains("../escape"));
    }
    let unknown =
        Harness::<SkylightPrivate>::get(component()).call("skylight-private.unknown", json!({}));
    assert!(
        unknown.is_err(),
        "unknown ID must be rejected at broker boundary"
    );
    for input in [r#"{"frameId":"one","frameId":"two"}"#, r#"{"frameId":{}}"#] {
        let actual = Native::<SkylightPrivate>::new().call(CATEGORIES, input);
        assert_eq!(actual.status, 2);
        assert_eq!(
            actual.stderr,
            "the input does not match the capability's input schema\n"
        );
    }
    Ok(())
}

#[test]
fn real_component_refuses_ephemeral_fixture_authority_before_http() -> TestResult {
    // Testkit's HTTPS server uses a non-443 port. No fixture may redirect the fixed private URL.
    for hostname in [HOST, "other.invalid"] {
        let fixture = HttpScript::new(hostname, "GET", response(account_fixture()));
        let harness = Harness::<SkylightPrivate>::get(component()).http(fixture);
        let origin = harness
            .origin()
            .expect("ephemeral HTTPS fixture")
            .to_owned();
        assert_ne!(origin, format!("https://{HOST}"));
        let Err(HarnessError::Invocation(failure)) = harness.call(ACCOUNT, json!({})) else {
            panic!("fixed private URI must not reach the ephemeral fixture");
        };
        assert!(format!("{:?}", failure.error).contains("HostCallRejected"));
        assert!(failure.http_calls.is_empty());
    }
    Ok(())
}

#[test]
fn host_does_not_reflect_http_failures_or_private_data() {
    let native = Native::<SkylightPrivate>::new().http(HttpScript::new(
        HOST,
        "GET",
        Response {
            status: 401,
            headers: vec![],
            body: b"private-credential-sentinel".to_vec(),
        },
    ));
    let result = native.call(ACCOUNT, "{}");
    assert_ne!(result.status, 0);
    assert!(result.stdout.is_empty());
    assert!(!result.stderr.contains("private-credential-sentinel"));
}
