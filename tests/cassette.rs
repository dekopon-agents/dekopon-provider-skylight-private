use dekopon_provider_sdk::provider::{Header, Response};
use dekopon_provider_sdk_testkit::{Harness, HttpScript, Native};
use dekopon_skylight_private_provider::SkylightPrivate;
use serde_json::{Value, json};

#[test]
fn authored_household_cassette_replays_with_exact_prefix_and_query()
-> Result<(), Box<dyn std::error::Error>> {
    let exchange: Value = serde_json::from_str(include_str!(
        "cassettes/skylight-private/0001-GET-calendar-events.json"
    ))?;
    assert_eq!(exchange["version"], 1);
    let response = Response {
        status: exchange["response"]["status"]
            .as_u64()
            .unwrap()
            .try_into()?,
        headers: vec![Header::text("content-type", "application/json")?],
        body: serde_json::to_vec(&exchange["response"]["body"]["json"])?,
    };
    let input = json!({"frameId":"frame-test","dateMin":"2028-03-11","dateMax":"2028-03-13","timezone":"Etc/GMT+5","include":"categories,calendar_account,event_notification_setting"});
    let capability = "skylight-private.calendar.events.list";
    let base = "https://fixture.example.test/skylight";
    let native = Native::<SkylightPrivate>::new()
        .settings(json!({"baseUrl":base}))
        .http(HttpScript::new(
            "fixture.example.test",
            "GET",
            response.clone(),
        ));
    let result = native.call(capability, &input.to_string());
    assert_eq!(result.status, 0, "{}", result.stderr);
    let sent = native.requests();
    assert_eq!(sent.len(), 1);
    let request = &exchange["request"];
    assert_eq!(sent[0].method, request["method"]);
    assert_eq!(
        sent[0].uri,
        format!(
            "{base}{}?{}",
            request["path"].as_str().unwrap(),
            request["query"].as_str().unwrap()
        )
    );
    assert!(sent[0].body.is_empty());
    let accept = sent[0]
        .headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("accept"))
        .unwrap();
    assert_eq!(
        accept.value,
        request["headers"]["accept"].as_str().unwrap().as_bytes()
    );
    assert!(
        !sent[0]
            .headers
            .iter()
            .any(|h| h.name.eq_ignore_ascii_case("authorization")
                || h.name.eq_ignore_ascii_case("cookie"))
    );
    let projected: Value = serde_json::from_slice(&result.stdout)?;
    assert_eq!(projected["events"].as_array().unwrap().len(), 1);
    assert_eq!(projected["events"][0]["id"], "event-synthetic");
    assert_eq!(
        projected["events"][0]["summary"],
        "Synthetic calendar event"
    );
    assert_eq!(projected["events"][0]["startsAt"], "2028-03-11");
    assert_eq!(projected["events"][0]["endsAt"], "2028-03-13");
    assert_eq!(projected["events"][0]["allDay"], true);
    assert_eq!(projected["truncated"], false);
    assert_eq!(projected["coverage"], "bounded-response");
    assert_eq!(projected["upstreamCompleteness"], "unknown");
    let component = std::env::var_os("DEKOPON_PROVIDER_COMPONENT")
        .expect("DEKOPON_PROVIDER_COMPONENT must name the freshly built component");
    let harness = Harness::<SkylightPrivate>::get(std::path::PathBuf::from(component))
        .http(HttpScript::new("localhost", "GET", response));
    let origin = harness.origin().unwrap().to_owned();
    let actual = harness
        .settings(json!({"baseUrl":format!("{origin}/skylight")}))
        .call(capability, input)?;
    assert_eq!(actual.status, 0, "{}", actual.stderr);
    assert_eq!(actual.http_calls.len(), 1);
    assert_eq!(serde_json::from_slice::<Value>(&actual.stdout)?, projected);
    Ok(())
}
