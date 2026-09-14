use super::household::*;
use super::*;

fn input(read: Read) -> Value {
    match read {
        Read::Categories | Read::Lists => json!({"frameId":"frame-test"}),
        Read::List | Read::Items => json!({"frameId":"frame-test", "listId":"list-test"}),
        Read::Events => {
            json!({"frameId":"frame-test", "dateMin":"2028-03-11", "dateMax":"2028-03-13", "timezone":"America/New_York"})
        }
    }
}
fn invoke(read: Read, body: Value) -> Result<Value, ProviderError> {
    invoke_raw(read, &body.to_string())
}
fn invoke_raw(read: Read, body: &str) -> Result<Value, ProviderError> {
    read.invoke(input(read), |request| {
        super::tests::assert_fixed_request(&request, &read.uri(&input(read)).unwrap());
        Ok(Response {
            status: 200,
            headers: vec![],
            body: body.as_bytes().to_vec(),
        })
    })
}

#[test]
fn household_routes_are_exact_gets_and_cli_matches_every_manifest_entry() {
    let cases = [
        (Read::Categories, "categories", "/categories"),
        (Read::Lists, "lists", "/lists"),
        (Read::List, "list-show", "/lists/list-test"),
        (Read::Items, "list-items", "/lists/list-test/list_items"),
        (
            Read::Events,
            "events",
            "/calendar_events?date_min=2028-03-11T00%3A00%3A00&date_max=2028-03-13T00%3A00%3A00&timezone=America%2FNew_York",
        ),
    ];
    for (read, verb, path) in cases {
        assert_eq!(
            read.uri(&input(read)).unwrap(),
            format!("{FRAMES_URI}/frame-test{path}")
        );
        let mut argv = vec![verb.to_owned()];
        for (flag, key) in read.fields().iter().rev() {
            argv.extend([
                (*flag).to_owned(),
                input(read)[key].as_str().unwrap().to_owned(),
            ]);
        }
        let CommandRun::Proposal(proposal) = commands::run(&argv, Some("secret-sentinel")) else {
            panic!("expected proposal");
        };
        assert_eq!(proposal.capability.as_str(), read.capability());
        assert_eq!(proposal.input, input(read));
        let output = invoke(
            read,
            if read == Read::List {
                json!({"data":{"id":"one"}})
            } else {
                json!({"data":[]})
            },
        )
        .unwrap();
        assert_eq!(output["coverage"], "bounded-response");
        assert_eq!(output["upstreamCompleteness"], "unknown");
        assert_eq!(output["truncated"], false);
        let manifest = read.manifest();
        assert_eq!(manifest.input_schema["additionalProperties"], false);
        assert_eq!(
            manifest.input_schema["required"].as_array().unwrap().len(),
            read.fields().len()
        );
    }
}

#[test]
fn household_inputs_and_cli_reject_transport_escape_hatches_and_duplicates() {
    for read in READS {
        for field in [
            "url",
            "endpoint",
            "host",
            "path",
            "query",
            "headers",
            "authorization",
            "bearer",
            "token",
            "body",
            "method",
            "include",
            "page",
            "cursor",
            "limit",
            "assignee",
            "completed",
            "categoryId",
        ] {
            let mut data = input(read);
            data[field] = json!("secret-sentinel");
            assert_eq!(
                read.invoke(data, |_| panic!("invalid input sent"))
                    .unwrap_err()
                    .code(),
                "invalid-input"
            );
        }
        for value in ["/", "%2F", "?", "#", "..", "a/b", "x\r\n", "", "é", "x.y"] {
            for field in ["frameId", "listId"] {
                let mut data = input(read);
                if data.get(field).is_none() {
                    continue;
                }
                data[field] = json!(value);
                assert!(read.uri(&data).is_err());
            }
        }
        for bad in [Value::Null, json!([]), json!({}), json!("x")] {
            assert!(read.uri(&bad).is_err());
        }
        for (_, field) in read.fields() {
            let mut data = input(read);
            data.as_object_mut().unwrap().remove(*field);
            assert!(read.uri(&data).is_err());
            let mut data = input(read);
            data[*field] = Value::Null;
            assert!(read.uri(&data).is_err());
        }
    }
    for raw in [
        r#"{"frameId":"a","frameId":"b"}"#,
        r#"{"frameId":null,"frameId":"b"}"#,
        r#"{"frameId":"a"} {}"#,
        "[]",
        "{broken",
    ] {
        assert!(parse_input(raw).is_err());
    }
    for words in [
        vec![
            "events",
            "--frame",
            "x",
            "--from",
            "2028-01-01",
            "--to",
            "2028-01-02",
        ],
        vec!["categories", "x"],
        vec!["lists", "--frame=x"],
        vec!["list-show", "--frame", "x", "--frame", "x"],
        vec!["list-items", "--frame", "x", "--list", "../secret-sentinel"],
        vec!["events", "--help"],
        vec!["tasks"],
        vec!["chores"],
        vec!["task-box"],
    ] {
        let argv: Vec<_> = words.into_iter().map(str::to_owned).collect();
        let CommandRun::Rendered {
            stdout,
            stderr,
            status,
        } = commands::run(&argv, None)
        else {
            panic!("invalid argv accepted");
        };
        assert_eq!(status, 2);
        assert!(stdout.is_empty());
        assert!(!stderr.contains("secret-sentinel"));
    }
}

#[test]
fn calendar_dates_leaps_bounds_timezones_and_dst_are_explicit() {
    for (from, to, valid) in [
        ("2028-02-29", "2028-03-01", true),
        ("2027-02-29", "2027-03-01", false),
        ("2000-02-29", "2000-03-01", true),
        ("2100-02-29", "2100-03-01", false),
        ("2028-01-01", "2028-02-01", true),
        ("2028-01-01", "2028-02-02", false),
        ("2028-01-02", "2028-01-01", false),
        ("2028-01-01", "2028-01-01", false),
        ("0000-01-01", "0000-01-02", false),
        ("2028-04-31", "2028-05-01", false),
        ("2028-00-01", "2028-01-01", false),
        ("2028-13-01", "2028-01-01", false),
        ("2028-01-00", "2028-01-01", false),
        ("2028-1-01", "2028-01-02", false),
        ("2028-01-01T00:00:00", "2028-01-02", false),
        ("😀-01-01", "2028-01-02", false),
        ("9999-12-30", "9999-12-31", true),
        ("2028-12-31", "2029-01-01", true),
    ] {
        let mut data = input(Read::Events);
        data["dateMin"] = json!(from);
        data["dateMax"] = json!(to);
        assert_eq!(Read::Events.uri(&data).is_ok(), valid, "{from} {to}");
    }
    for zone in [
        "",
        "../UTC",
        "UTC?include=x",
        "UTC#x",
        "UTC\r\n",
        "A//B",
        "/UTC",
        "UTC/",
        "America/New York",
        &"x".repeat(129),
    ] {
        let mut data = input(Read::Events);
        data["timezone"] = json!(zone);
        assert!(Read::Events.uri(&data).is_err());
    }
    let mut data = input(Read::Events);
    data["timezone"] = json!("Etc/GMT+5");
    assert!(
        Read::Events
            .uri(&data)
            .unwrap()
            .ends_with("timezone=Etc%2FGMT%2B5")
    );
    assert!(
        Read::Events
            .uri(&input(Read::Events))
            .unwrap()
            .contains("2028-03-13T00%3A00%3A00")
    );
}

#[test]
fn calendar_preserves_source_times_unknown_all_day_and_no_person_or_trip_inference() {
    let output = invoke(Read::Events, json!({"data":[
        {"id":"a", "attributes":{"summary":"Possible trip", "starts_at":"2028-03-10T23:00:00-05:00", "ends_at":"2028-03-14T01:00:00-04:00", "all_day":false, "category_ids":["person-secret-sentinel"], "completed":true}, "relationships":{"assignee":"secret-sentinel"}},
        {"id":"b", "attributes":{"all_day":true,"starts_at":"2028-03-11","ends_at":"2028-03-14"}},
        {"id":"c", "attributes":{"all_day":null}}, {"id":"d"}
    ], "links":{"next":"https://secret-sentinel.invalid"}, "meta":{"total":100}, "included":[{"bearer":"secret-sentinel"}]})).unwrap();
    let events = &output["events"];
    assert_eq!(events[0]["startsAt"], "2028-03-10T23:00:00-05:00");
    assert_eq!(events[0]["endsAt"], "2028-03-14T01:00:00-04:00");
    assert_eq!(events[0]["allDay"], false);
    assert_eq!(events[1]["allDay"], true);
    for i in [2, 3] {
        assert!(events[i]["allDay"].is_null());
        assert!(events[i]["startsAt"].is_null());
    }
    assert!(!output.to_string().contains("secret-sentinel"));
    assert!(events[0].get("assignee").is_none());
    assert!(events[0].get("completed").is_none());
    assert_eq!(output["upstreamCompleteness"], "unknown");
    for id in [
        "skylight.private.tasks.list",
        "skylight.private.chores.list",
        "skylight.private.task.box.list",
    ] {
        assert!(Read::from_capability(id).is_none());
        assert!(
            matches!(invoke_component(id,"{broken"),ComponentResponse::Failed {error} if error.code == "unknown-capability")
        );
    }
}

#[test]
fn list_item_status_is_not_dated_assigned_outstanding_tasks() {
    let output = invoke(Read::Items, json!({"data":[
        {"id":"a","attributes":{"label":"Sample item", "status":"pending","section":"Sample section","due":"secret-sentinel","assignee":"secret-sentinel"}},
        {"id":"b","attributes":{"status":"completed"}},
        {"id":"c","attributes":{"status":"new-unknown-status"}},
        {"id":"d","attributes":{"status":null}}, {"id":"e"}
    ]})).unwrap();
    assert_eq!(output["items"][0]["status"], "pending");
    assert_eq!(output["items"][1]["status"], "completed");
    for i in 2..5 {
        assert!(output["items"][i]["status"].is_null());
    }
    assert!(!output.to_string().contains("secret-sentinel"));
    let categories = invoke(Read::Categories, json!({"data":[{"id":"a","attributes":{"label":"Same"}},{"id":"b","attributes":{"label":"Same"}}]})).unwrap();
    assert_eq!(categories["categories"].as_array().unwrap().len(), 2);
    assert!(categories["categories"][0].get("person").is_none());
}

#[test]
fn household_known_fields_duplicates_nulls_and_malformed_tails_fail_closed() {
    for read in READS {
        let resource = if read == Read::Events {
            r#"{"id":"a","attributes":{"all_day":null,"all_day":true}}"#
        } else {
            r#"{"id":"a","attributes":{"label":null,"label":"x"}}"#
        };
        for raw in [
            format!(r#"{{"data":{resource}}}"#),
            format!(r#"{{"data":[{resource}]}}"#),
            r#"{"data":[],"data":[]}"#.to_owned(),
            r#"{"data":[{"id":"a"},{"id":"a"}]}"#.to_owned(),
            r#"{"data":[{"id":"a","attributes":null}]}"#.to_owned(),
            r#"{"data":[["a",{}]]}"#.to_owned(),
        ] {
            assert_eq!(
                invoke_raw(read, &raw).unwrap_err().code(),
                "invalid-response"
            );
        }
    }
    for (read, attributes) in [
        (Read::Events, json!({"all_day":"false"})),
        (Read::Events, json!({"starts_at":7})),
        (Read::Events, json!({"ends_at":"x".repeat(257)})),
        (Read::Items, json!({"status":false})),
        (Read::Items, json!({"section":[]})),
        (Read::Lists, json!({"label":1})),
    ] {
        let mut records: Vec<_> = (0..70).map(|i| json!({"id":format!("{i:03}")})).collect();
        records.push(json!({"id":"z","attributes":attributes}));
        assert_eq!(
            invoke(read, json!({"data":records})).unwrap_err().code(),
            "invalid-response"
        );
    }
}

#[test]
fn household_typed_retention_preserves_fields_independent_of_input_order() {
    for (read, key) in [
        (Read::Categories, "categories"),
        (Read::Events, "events"),
        (Read::Lists, "lists"),
        (Read::Items, "items"),
    ] {
        let records: Vec<_> = (0..90)
            .map(|i| {
                json!({"id":format!("{i:03}"), "attributes":{
                    "label":format!("Label {i}"), "summary":format!("Event {i}"),
                    "starts_at":"2028-03-11", "ends_at":"2028-03-12", "all_day":i % 2 == 0,
                    "status":if i % 2 == 0 {"pending"} else {"completed"},
                    "section":format!("Section {i}")
                }})
            })
            .collect();
        let ascending = invoke(read, json!({"data":records})).unwrap();
        let descending: Vec<_> = records.iter().rev().collect();
        let permuted: Vec<_> = (0..90).map(|i| &records[(i * 31 + 17) % 90]).collect();
        assert_eq!(ascending, invoke(read, json!({"data":descending})).unwrap());
        assert_eq!(ascending, invoke(read, json!({"data":permuted})).unwrap());
        let selected = ascending[key].as_array().unwrap();
        assert_eq!(selected.len(), 64);
        for (index, record) in selected.iter().enumerate() {
            let single = invoke(read, json!({"data":[records[index]]})).unwrap();
            assert_eq!(*record, single[key][0]);
        }
    }
}

#[test]
fn household_count_text_byte_response_limits_and_sanitized_statuses() {
    let records: Vec<_> = (0..70)
        .rev()
        .map(|i| json!({"id":format!("{i:03}"),"attributes":{"label":"é".repeat(200)}}))
        .collect();
    let output = invoke(Read::Lists, json!({"data":records})).unwrap();
    assert_eq!(output["lists"].as_array().unwrap().len(), 64);
    assert_eq!(output["lists"][0]["id"], "000");
    assert_eq!(output["truncated"], true);
    assert!(output["lists"][0]["label"].as_str().unwrap().ends_with('…'));
    let records: Vec<_> = (0..64).map(|i|json!({"id":format!("{i:03}{}","\0".repeat(125)),"attributes":{"summary":"\0".repeat(256),"starts_at":"\0".repeat(256),"ends_at":"\0".repeat(256)}})).collect();
    // Large escaping can consume the response budget as well; choose a response-bounded subset.
    let output = invoke(Read::Events, json!({"data":&records[..40]})).unwrap();
    assert!(output["events"].as_array().unwrap().len() < 40);
    assert_eq!(output["truncated"], true);
    assert!(
        serde_json::to_vec(&ComponentResponse::Succeeded { output })
            .unwrap()
            .len()
            < MAX_COMPONENT_OUTPUT_BYTES
    );
    for read in READS {
        assert_eq!(
            invoke_raw(read, &"x".repeat(MAX_RESPONSE_BODY_BYTES + 1))
                .unwrap_err()
                .code(),
            "invalid-response"
        );
        for status in [301, 302, 401, 403, 404, 429, 500] {
            let error = read
                .invoke(input(read), |_| {
                    Ok(Response {
                        status,
                        headers: vec![
                            Header::text("location", "https://secret-sentinel.invalid").unwrap(),
                        ],
                        body: b"secret-sentinel".to_vec(),
                    })
                })
                .unwrap_err();
            assert_eq!(error.code(), status_error(status).code());
            assert!(!error.message().contains("secret-sentinel"));
        }
    }
}
