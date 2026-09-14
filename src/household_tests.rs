use super::household::*;
use super::*;

fn input(read: Read) -> Value {
    match read {
        Read::Categories | Read::Lists => json!({"frameId":"frame-test"}),
        Read::List | Read::Items => json!({"frameId":"frame-test", "listId":"list-test"}),
        Read::Tasks => json!({"frameId":"frame-test", "after":"2028-03-11", "before":"2028-03-11"}),
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
        (
            Read::Tasks,
            "tasks",
            "/chores?after=2028-03-11&before=2028-03-11&include_late=false&include_up_for_grabs=false&filter=linked_to_profile",
        ),
        (Read::Categories, "categories", "/categories"),
        (Read::Lists, "lists", "/lists"),
        (Read::List, "list-show", "/lists/list-test"),
        (Read::Items, "list-items", "/lists/list-test/list_items"),
        (
            Read::Events,
            "events",
            "/calendar_events?date_min=2028-03-11&date_max=2028-03-13&timezone=America%2FNew_York",
        ),
    ];
    for (read, verb, path) in cases {
        assert_eq!(
            read.uri(&input(read)).unwrap(),
            format!("{FRAMES_URI}/frame-test{path}")
        );
        let mut argv = vec![verb.to_owned()];
        for (flag, key) in read.fields().iter().rev().filter(|(_, k)| read.required(k)) {
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
            read.fields()
                .iter()
                .filter(|(_, k)| read.required(k))
                .count()
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
        for (_, field) in read.fields().iter().filter(|(_, k)| read.required(k)) {
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
            .contains("2028-03-13")
    );
}

#[test]
fn calendar_preserves_source_times_unknown_all_day_and_no_person_or_trip_inference() {
    let output = invoke(Read::Events, json!({"data":[
        {"id":"a", "attributes":{"summary":"Possible trip", "starts_at":"2028-03-10T23:00:00-05:00", "ends_at":"2028-03-14T01:00:00-04:00", "all_day":false, "category_ids":["person-secret-sentinel"], "completed":true}, "relationships":{"assignee":"secret-sentinel"}},
        {"id":"b", "attributes":{"all_day":true,"starts_at":"2028-03-11","ends_at":"2028-03-14"}},
        {"id":"c", "attributes":{"all_day":null}}, {"id":"d"}
    ], "links":{"next":"https://secret-sentinel.invalid"}, "meta":{"total":100}, "included":[{"id":"stub","type":"unknown","attributes":{"bearer":"secret-sentinel"}}]})).unwrap();
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
        } else if read == Read::Tasks {
            r#"{"id":"a","attributes":{"status":null,"status":"pending"}}"#
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
        (Read::Tasks, "tasks"),
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

#[test]
fn tasks_and_event_optional_parameters_match_schema_cli_and_raw_json() {
    let event_include = "categories,calendar_account,event_notification_setting";
    let mut events = input(Read::Events);
    assert!(!Read::Events.uri(&events).unwrap().contains("include="));
    let mut tasks = input(Read::Tasks);
    tasks["includeLate"] = json!(false);
    tasks["includeUpForGrabs"] = json!(false);
    tasks["filter"] = json!("linked_to_profile");
    let default_uri = Read::Tasks.uri(&input(Read::Tasks)).unwrap();
    assert_eq!(Read::Tasks.uri(&tasks).unwrap(), default_uri);
    for optional in ["includeLate", "includeUpForGrabs", "filter"] {
        let mut omitted = tasks.clone();
        omitted.as_object_mut().unwrap().remove(optional);
        assert_eq!(Read::Tasks.uri(&omitted).unwrap(), default_uri);
    }
    events["include"] = json!(event_include);
    assert!(
        Read::Events
            .uri(&events)
            .unwrap()
            .ends_with("&include=categories%2Ccalendar_account%2Cevent_notification_setting")
    );
    assert_eq!(
        Read::Events.manifest().input_schema["properties"]["include"]["enum"],
        json!([event_include])
    );
    for bad in [
        "",
        "category",
        "categories",
        "categories,calendar_account",
        "categories,categories",
        "calendar_account,categories,event_notification_setting",
        "x&url=https://invalid",
    ] {
        events["include"] = json!(bad);
        assert!(Read::Events.uri(&events).is_err());
    }
    for late in [false, true] {
        for grabs in [false, true] {
            let raw = format!(
                r#"{{"frameId":"frame-test","after":"2028-03-11","before":"2028-03-11","includeLate":{late},"includeUpForGrabs":{grabs},"filter":"linked_to_profile"}}"#
            );
            let data = parse_input(&raw).unwrap();
            assert!(Read::Tasks.uri(&data).unwrap().ends_with(&format!(
                "include_late={late}&include_up_for_grabs={grabs}&filter=linked_to_profile"
            )));
            let args = [
                "tasks",
                "--frame",
                "frame-test",
                "--after",
                "2028-03-11",
                "--before",
                "2028-03-11",
                "--include-late",
                if late { "true" } else { "false" },
                "--include-up-for-grabs",
                if grabs { "true" } else { "false" },
                "--filter",
                "linked_to_profile",
            ]
            .map(str::to_owned);
            let CommandRun::Proposal(proposal) = commands::run(&args, None) else {
                panic!("expected typed proposal");
            };
            assert_eq!(proposal.input, data);
        }
    }
    for (field, bad) in [
        ("includeLate", json!("false")),
        ("includeUpForGrabs", json!(0)),
        ("filter", json!("all")),
        ("filter", Value::Null),
        ("includeLate", Value::Null),
    ] {
        let mut data = input(Read::Tasks);
        data[field] = bad;
        assert!(
            Read::Tasks
                .invoke(data, |_| panic!("invalid input sent"))
                .is_err()
        );
    }
    for raw in [
        r#"{"includeLate":false,"includeLate":true}"#,
        r#"{"filter":"linked_to_profile","filter":"linked_to_profile"}"#,
        r#"{"frameId":{},"frameId":"x"}"#,
    ] {
        assert!(parse_input(raw).is_err());
    }
    for (after, before, valid) in [
        ("2028-02-29", "2028-02-29", true),
        ("2027-02-29", "2027-02-29", false),
        ("2028-03-11", "2028-03-12", true),
        ("2028-03-01", "2028-04-01", true),
        ("2028-03-01", "2028-04-02", false),
        ("2028-03-12", "2028-03-11", false),
        ("2028-03-11T00:00:00", "2028-03-12", false),
    ] {
        let data = json!({"frameId":"frame-test", "after":after, "before":before});
        assert_eq!(Read::Tasks.uri(&data).is_ok(), valid);
    }
    assert_eq!(
        Read::Tasks.manifest().input_schema["properties"]["includeLate"],
        json!({"type":"boolean","default":false})
    );
}

#[test]
fn tasks_retain_source_status_recurrence_and_completion_without_outstanding_inference() {
    for status in ["pending", "complete", "skipped", "future-status"] {
        let output = invoke(Read::Tasks, json!({"data":[{"id":"task-test","type":"chore","attributes":{
            "id":23,"group":"group-test","series":"series-test","summary":"Synthetic task","description":null,"status":status,
            "start":"2028-03-11","completed_at":"opaque completion string","completed_on":{"unverified":"secret-sentinel"},
            "recurring_until":null,"start_time":null,"recurrence_set":["RRULE:FREQ=DAILY"],"recurring":true,"routine":false,"up_for_grabs":true,"position":2
        }}]})).unwrap();
        let task = &output["tasks"][0];
        assert_eq!(task["status"], status);
        assert_eq!(task["group"], "group-test");
        assert_eq!(task["series"], "series-test");
        assert_eq!(task["completedAt"], "opaque completion string");
        assert_eq!(task["completedOnState"], "unverified-non-null");
        assert_eq!(task["recurringUntilState"], "null-or-missing");
        assert_eq!(task["recurrenceSet"], json!(["RRULE:FREQ=DAILY"]));
        assert_eq!(task["upForGrabs"], true);
        assert!(task.get("outstanding").is_none());
        assert!(!output.to_string().contains("secret-sentinel"));
        assert_eq!(output["upstreamCompleteness"], "unknown");
    }
    let empty = invoke(Read::Tasks, json!({"data":[{"id":"task-test"}]})).unwrap();
    for field in [
        "status",
        "completedAt",
        "recurrenceSet",
        "recurring",
        "start",
    ] {
        assert!(empty["tasks"][0][field].is_null());
    }
}

#[test]
fn relationships_resolve_included_by_type_and_id_without_assuming_categories_are_people() {
    let output = invoke(Read::Tasks, json!({"data":[{"id":"task-test","relationships":{
        "category":{"data":{"type":"category","id":"cat-test"}},
        "completed_category":{"data":{"type":"category","id":"completion-test"}}
    }}],"included":[
        {"type":"category","id":"cat-test","attributes":{"label":"Synthetic category","linked_to_profile":true},"relationships":{"family_member":{"data":{"type":"family_member","id":"member-test"}}}},
        {"type":"family_member","id":"member-test","attributes":{"unverified":"secret-sentinel","name":"secret-sentinel"}},
        {"type":"other","id":"completion-test"}
    ]})).unwrap();
    let relationships = &output["tasks"][0]["relationships"];
    assert_eq!(relationships["category"]["data"]["included"], true);
    assert_eq!(
        relationships["completed_category"]["data"]["included"],
        false
    );
    let category = output["included"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["type"] == "category")
        .unwrap();
    assert_eq!(
        category["relationships"]["family_member"]["data"]["included"],
        true
    );
    assert!(!output.to_string().contains("secret-sentinel"));
    assert!(category.get("person").is_none());

    let events = invoke(Read::Events, json!({"data":[{"id":"event-test","relationships":{
        "categories":{"data":[{"type":"category","id":"b"},{"type":"category","id":"a"}]},
        "calendar_account":{"data":null},"event_notification_setting":{"data":{"type":"event_notification_setting","id":"setting-test"}}
    }}],"included":[{"type":"category","id":"a"},{"type":"category","id":"b","relationships":{"family_member":{"data":null}}},{"type":"event_notification_setting","id":"setting-test"}]})).unwrap();
    let links = &events["events"][0]["relationships"]["categories"]["data"];
    assert_eq!(links[0]["id"], "a");
    assert_eq!(links[1]["id"], "b");
    assert_eq!(links[0]["included"], true);
    assert!(events["events"][0]["relationships"]["calendar_account"]["data"].is_null());
    assert_eq!(events["included"].as_array().unwrap().len(), 3);
}

#[test]
fn list_detail_hydrates_items_and_reports_unverified_sections_without_forwarding_them() {
    let output = invoke(Read::List, json!({"data":{"id":"list-test","type":"list","attributes":{"label":"Synthetic list","kind":"unknown-kind","color":"color-test","hide_on_device":false,"draft":false,"default_grocery_list":true},"relationships":{"list_items":{"data":[{"type":"list_item","id":"item-test"}]}}},"included":[{"id":"item-test","type":"list_item","attributes":{"label":"Synthetic item","status":"future-status","section":null,"position":1,"draft":false,"created_at":"opaque time"},"relationships":{"list":{"data":{"id":"list-test","type":"list"}}}}],"meta":{"sections":[{"unknown":"secret-sentinel"}]}})).unwrap();
    assert_eq!(output["list"]["defaultGroceryList"], true);
    assert_eq!(
        output["list"]["relationships"]["list_items"]["data"][0]["included"],
        true
    );
    assert_eq!(output["included"][0]["status"], "future-status");
    assert_eq!(
        output["included"][0]["relationships"]["list"]["data"]["included"],
        false
    );
    assert_eq!(output["sectionsCount"], 1);
    assert_eq!(output["sectionsSchema"], "unknown");
    assert_eq!(output["truncated"], true);
    assert!(!output.to_string().contains("secret-sentinel"));
    let empty = invoke(
        Read::List,
        json!({"data":{"id":"list-test"},"meta":{"sections":[]},"included":[]}),
    )
    .unwrap();
    assert_eq!(empty["sectionsCount"], 0);
    assert_eq!(empty["includedState"], "present");
    assert_eq!(empty["truncated"], false);
}

#[test]
fn relationship_recurrence_and_included_tails_are_validated_before_local_bounds() {
    for raw in [
        r#"{"data":[{"id":"a","relationships":{"category":{"data":null,"data":null}}}]}"#,
        r#"{"data":[{"id":"a","relationships":{"category":null,"category":{"data":null}}}]}"#,
        r#"{"data":[{"id":"a","relationships":{"categories":{"data":{}}}}]}"#,
        r#"{"data":[{"id":"a","relationships":{"category":{"data":[]}}}]}"#,
        r#"{"data":[{"id":"a","relationships":{"category":{"data":{"id":"x","type":"category","type":"category"}}}}]}"#,
        r#"{"data":[{"id":"a","relationships":{"categories":{"data":[{"id":"x","type":"category"},{"id":"x","type":"category"}]}}}]}"#,
        r#"{"data":[],"included":null,"included":[]}"#,
        r#"{"data":[],"included":[{"id":"x"}]}"#,
        r#"{"data":[],"included":[{"id":"x","type":"category"},{"id":"x","type":"category"}]}"#,
        r#"{"data":[],"meta":{"sections":null,"sections":[]}}"#,
        r#"{"data":[],"meta":{"sections":{}}}"#,
        r#"{"data":[{"id":"x","attributes":{"completed_on":null,"completed_on":null}}]}"#,
    ] {
        assert_eq!(
            invoke_raw(Read::Tasks, raw).unwrap_err().code(),
            "invalid-response",
            "{raw}"
        );
    }
    for attrs in [
        json!({"recurrence_set":[1]}),
        json!({"recurring":"true"}),
        json!({"position":"one"}),
        json!({"completed_at":7}),
        json!({"start":"x".repeat(257)}),
    ] {
        let mut data: Vec<_> = (0..70).map(|i| json!({"id":format!("{i:03}")})).collect();
        data.push(json!({"id":"tail","attributes":attrs}));
        assert!(invoke(Read::Tasks, json!({"data":data})).is_err());
    }
    let mut links: Vec<_> = (0..20)
        .rev()
        .map(|i| json!({"id":format!("{i:03}"),"type":"category"}))
        .collect();
    let valid = invoke(
        Read::Events,
        json!({"data":[{"id":"event","relationships":{"categories":{"data":links}}}]}),
    )
    .unwrap();
    assert_eq!(
        valid["events"][0]["relationships"]["categories"]["data"]
            .as_array()
            .unwrap()
            .len(),
        16
    );
    assert_eq!(
        valid["events"][0]["relationships"]["categories"]["data"][0]["id"],
        "000"
    );
    assert_eq!(valid["truncated"], true);
    links.push(json!({"id":"tail","type":1}));
    assert!(
        invoke(
            Read::Events,
            json!({"data":[{"id":"event","relationships":{"categories":{"data":links}}}]})
        )
        .is_err()
    );
    let mut rules: Vec<_> = (0..20).map(|i| json!(format!("RRULE:TEST={i}"))).collect();
    let valid = invoke(
        Read::Tasks,
        json!({"data":[{"id":"task","attributes":{"recurrence_set":rules}}]}),
    )
    .unwrap();
    assert_eq!(
        valid["tasks"][0]["recurrenceSet"].as_array().unwrap().len(),
        16
    );
    assert_eq!(valid["tasks"][0]["recurrenceSetTruncated"], true);
    assert_eq!(valid["tasks"][0]["textTruncated"], false);
    assert_eq!(valid["truncated"], true);
    rules.push(json!(false));
    assert!(
        invoke(
            Read::Tasks,
            json!({"data":[{"id":"task","attributes":{"recurrence_set":rules}}]})
        )
        .is_err()
    );
    let mut included: Vec<_> = (0..70)
        .rev()
        .map(|i| json!({"id":format!("{i:03}"),"type":"category"}))
        .collect();
    let valid = invoke(Read::Tasks, json!({"data":[],"included":included})).unwrap();
    assert_eq!(valid["included"].as_array().unwrap().len(), 64);
    assert_eq!(valid["truncated"], true);
    included.push(json!({"id":"tail","type":"category","attributes":{"label":false}}));
    assert!(invoke(Read::Tasks, json!({"data":[],"included":included})).is_err());
}
