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
    for (name, value) in request["headers"].as_object().unwrap() {
        let sent_header = sent[0]
            .headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case(name))
            .unwrap();
        assert_eq!(sent_header.value, value.as_str().unwrap().as_bytes());
    }
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

#[test]
fn authored_write_cassettes_replay_with_exact_method_prefix_headers_and_body()
-> Result<(), Box<dyn std::error::Error>> {
    let base = "https://fixture.example.test/skylight";
    for raw in [
        include_str!("cassettes/skylight-private/0002-POST-calendar-events.json"),
        include_str!("cassettes/skylight-private/0003-PUT-calendar-event.json"),
        include_str!("cassettes/skylight-private/0004-DELETE-calendar-event.json"),
        include_str!("cassettes/skylight-private/0005-POST-list-items.json"),
        include_str!("cassettes/skylight-private/0006-PUT-list-item.json"),
        include_str!("cassettes/skylight-private/0007-PUT-list-item-complete.json"),
        include_str!("cassettes/skylight-private/0008-DELETE-list-item.json"),
        include_str!("cassettes/skylight-private/0009-POST-lists.json"),
        include_str!("cassettes/skylight-private/0010-PUT-list.json"),
        include_str!("cassettes/skylight-private/0011-DELETE-list.json"),
        include_str!("cassettes/skylight-private/0012-POST-chores.json"),
        include_str!("cassettes/skylight-private/0013-PUT-chore.json"),
        include_str!("cassettes/skylight-private/0014-PUT-chore-completions.json"),
        include_str!("cassettes/skylight-private/0015-DELETE-chore.json"),
        include_str!("cassettes/skylight-private/0016-POST-categories.json"),
        include_str!("cassettes/skylight-private/0017-POST-categories-find-or-create.json"),
        include_str!("cassettes/skylight-private/0018-PATCH-category.json"),
        include_str!("cassettes/skylight-private/0019-DELETE-category.json"),
    ] {
        let exchange: Value = serde_json::from_str(raw)?;
        assert_eq!(exchange["version"], 1);
        assert!(
            exchange["provenance"]
                .as_str()
                .unwrap()
                .starts_with("synthetic")
        );
        let capability = exchange["capability"].as_str().unwrap();
        let request = &exchange["request"];
        let method = request["method"].as_str().unwrap();
        let reply = &exchange["response"];
        let response = Response {
            status: reply["status"].as_u64().unwrap().try_into()?,
            headers: reply["headers"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(name, value)| Header::text(name.clone(), value.as_str().unwrap()))
                .collect::<Result<_, _>>()?,
            body: match &reply["body"] {
                Value::Null => Vec::new(),
                body => serde_json::to_vec(&body["json"])?,
            },
        };
        let input = exchange["input"].to_string();
        let native = Native::<SkylightPrivate>::new()
            .settings(json!({"baseUrl":base}))
            .http(HttpScript::new(
                "fixture.example.test",
                method,
                response.clone(),
            ));
        let result = native.call(capability, &input);
        assert_eq!(result.status, 0, "{capability}: {}", result.stderr);
        let sent = native.requests();
        assert_eq!(sent.len(), 1, "{capability}");
        assert_eq!(sent[0].method, method);
        let query = request["query"]
            .as_str()
            .map_or(String::new(), |query| format!("?{query}"));
        assert_eq!(
            sent[0].uri,
            format!("{base}{}{query}", request["path"].as_str().unwrap())
        );
        match &request["body"] {
            Value::Null => assert!(sent[0].body.is_empty(), "{capability}"),
            body => assert_eq!(
                serde_json::from_slice::<Value>(&sent[0].body)?,
                body["json"],
                "{capability}"
            ),
        }
        for (name, value) in request["headers"].as_object().unwrap() {
            let sent_header = sent[0]
                .headers
                .iter()
                .find(|h| h.name.eq_ignore_ascii_case(name))
                .unwrap();
            assert_eq!(sent_header.value, value.as_str().unwrap().as_bytes());
        }
        assert!(
            !sent[0]
                .headers
                .iter()
                .any(|h| h.name.eq_ignore_ascii_case("authorization")
                    || h.name.eq_ignore_ascii_case("cookie"))
        );
        let projected: Value = serde_json::from_slice(&result.stdout)?;
        if reply["status"] == 204 {
            assert_eq!(projected, json!({"ok": true}));
        } else {
            let record = &reply["body"]["json"]["data"];
            let attributes = &record["attributes"];
            if capability.contains(".calendar.events.") {
                assert_eq!(projected["event"]["id"], record["id"]);
                assert_eq!(projected["event"]["summary"], attributes["summary"]);
                assert_eq!(projected["event"]["startsAt"], attributes["starts_at"]);
            } else if capability.contains(".tasks.") {
                assert_eq!(projected["task"]["id"], record["id"]);
                assert_eq!(projected["task"]["summary"], attributes["summary"]);
                assert_eq!(projected["task"]["status"], attributes["status"]);
            } else if capability.contains(".categories.") {
                assert_eq!(projected["category"]["id"], record["id"]);
                assert_eq!(projected["category"]["label"], attributes["label"]);
                assert_eq!(projected["category"]["color"], attributes["color"]);
            } else if capability.contains(".lists.") {
                assert_eq!(projected["list"]["id"], record["id"]);
                assert_eq!(projected["list"]["label"], attributes["label"]);
                assert_eq!(projected["list"]["kind"], attributes["kind"]);
            } else {
                assert_eq!(projected["item"]["id"], record["id"]);
                assert_eq!(projected["item"]["label"], attributes["label"]);
                assert_eq!(projected["item"]["status"], attributes["status"]);
            }
        }
        let component = std::env::var_os("DEKOPON_PROVIDER_COMPONENT")
            .expect("DEKOPON_PROVIDER_COMPONENT must name the freshly built component");
        let harness = Harness::<SkylightPrivate>::get(std::path::PathBuf::from(component))
            .http(HttpScript::new("localhost", method, response));
        let origin = harness.origin().unwrap().to_owned();
        let actual = harness
            .settings(json!({"baseUrl":format!("{origin}/skylight")}))
            .call(capability, exchange["input"].clone())?;
        assert_eq!(actual.status, 0, "{capability}: {}", actual.stderr);
        assert_eq!(actual.http_calls.len(), 1);
        assert_eq!(actual.http_calls[0].method, method);
        assert_eq!(serde_json::from_slice::<Value>(&actual.stdout)?, projected);
    }
    Ok(())
}
