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
        Read::Messages | Read::Albums | Read::MonthInReview | Read::MonthInReviews => {
            json!({"frameId":"frame-test"})
        }
        Read::Message | Read::MessageLikes | Read::MessageComments => {
            json!({"frameId":"frame-test", "messageId":"message-test"})
        }
        Read::AlbumMessages | Read::AlbumMessageIds => {
            json!({"frameId":"frame-test", "albumId":"album-test"})
        }
    }
}
fn invoke(read: Read, body: Value) -> Result<Value, ProviderError> {
    invoke_raw(read, &body.to_string())
}
fn invoke_raw(read: Read, body: &str) -> Result<Value, ProviderError> {
    read.invoke(input(read), |request| {
        super::tests::assert_fixed_request(
            &request,
            &DEFAULT_BASE.join(&read.uri(&input(read)).unwrap()).unwrap(),
        );
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
        let dekopon_provider_sdk::CommandRunOutcome::Proposed {
            capability,
            input: proposed,
            ..
        } = provider::command::<SkylightPrivate>(&argv, true)
        else {
            panic!("expected proposal");
        };
        assert_eq!(capability.as_str(), read.capability());
        assert_eq!(proposed, input(read));
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
        vec!["list-show", "--frame", "x", "--frame", "x"],
        vec!["list-items", "--frame", "x", "--list", "../secret-sentinel"],
        vec!["tasks"],
        vec!["chores"],
        vec!["task-box"],
    ] {
        let argv: Vec<_> = words.into_iter().map(str::to_owned).collect();
        let outcome = provider::command::<SkylightPrivate>(&argv, false);
        assert!(
            !matches!(
                outcome,
                dekopon_provider_sdk::CommandRunOutcome::Proposed { .. }
            ),
            "invalid argv accepted"
        );
        assert!(!format!("{outcome:?}").contains("secret-sentinel"));
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
        let output =
            dekopon_provider_sdk_testkit::Native::<SkylightPrivate>::new().call(id, "{broken");
        assert_eq!(output.status, 1);
        assert!(
            output
                .stderr
                .contains("the provider has no such capability")
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
    assert!(serde_json::to_vec(&output).unwrap().len() < MAX_COMPONENT_OUTPUT_BYTES);
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
            let dekopon_provider_sdk::CommandRunOutcome::Proposed { input, .. } =
                provider::command::<SkylightPrivate>(&args, false)
            else {
                panic!("expected typed proposal");
            };
            assert_eq!(input, data);
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
        provider::manifest::<SkylightPrivate>()
            .unwrap()
            .capabilities[7]
            .input_schema["properties"]["includeLate"]["type"],
        json!("boolean")
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

fn create_input() -> Value {
    json!({
        "frameId": "frame-test",
        "summary": "Dentist",
        "startsAt": "2028-03-11T09:00:00-05:00",
        "endsAt": "2028-03-11T10:00:00-05:00",
        "allDay": false,
        "timezone": "America/New_York",
        "categoryIds": ["category-1", "category-2"],
        "rrule": ["RRULE:FREQ=WEEKLY;COUNT=2"]
    })
}
fn write_input(write: Mutation) -> Value {
    match write {
        Mutation::EventCreate => create_input(),
        Mutation::EventUpdate => {
            json!({"frameId":"frame-test", "eventId":"event-7", "summary":"Dentist moved", "allDay":true})
        }
        Mutation::EventDelete => json!({"frameId":"frame-test", "eventId":"event-7"}),
        Mutation::ItemCreate => {
            json!({"frameId":"frame-test", "listId":"list-test", "label":"Milk", "section":"Dairy"})
        }
        Mutation::ItemUpdate => {
            json!({"frameId":"frame-test", "listId":"list-test", "itemId":"item-7", "label":"Oat milk", "status":"pending", "position":3})
        }
        Mutation::ItemComplete => {
            json!({"frameId":"frame-test", "listId":"list-test", "itemId":"item-7"})
        }
        Mutation::ItemDelete => {
            json!({"frameId":"frame-test", "listId":"list-test", "itemId":"item-7"})
        }
        Mutation::ListCreate => {
            json!({"frameId":"frame-test", "label":"Groceries", "color":"#2E7D32", "kind":"shopping"})
        }
        Mutation::ListUpdate => {
            json!({"frameId":"frame-test", "listId":"list-test", "label":"Chores", "kind":"to_do"})
        }
        Mutation::ListDelete => json!({"frameId":"frame-test", "listId":"list-test"}),
        Mutation::CategoryCreate | Mutation::CategoryFindOrCreate => {
            json!({"frameId":"frame-test", "label":"Swim", "color":"#1E88E5"})
        }
        Mutation::CategoryUpdate => {
            json!({"frameId":"frame-test", "categoryId":"category-7", "label":"Swimming"})
        }
        Mutation::CategoryDelete => {
            json!({"frameId":"frame-test", "categoryId":"category-7", "reassignToId":"category-8"})
        }
    }
}
/// Every selector a write's route takes, and its required non-selector field.
fn write_required(write: Mutation) -> &'static [&'static str] {
    match write {
        Mutation::EventCreate => &["frameId", "summary"],
        Mutation::EventUpdate | Mutation::EventDelete => &["frameId", "eventId"],
        Mutation::ItemCreate => &["frameId", "listId", "label"],
        Mutation::ListCreate => &["frameId", "label"],
        Mutation::ListUpdate | Mutation::ListDelete => &["frameId", "listId"],
        Mutation::CategoryCreate | Mutation::CategoryFindOrCreate => &["frameId", "label", "color"],
        Mutation::CategoryUpdate | Mutation::CategoryDelete => &["frameId", "categoryId"],
        _ => &["frameId", "listId", "itemId"],
    }
}
/// The returned record and its output key for each resource.
fn write_record(write: Mutation) -> (&'static str, &'static str, Value) {
    match write {
        Mutation::EventCreate | Mutation::EventUpdate | Mutation::EventDelete => (
            "event",
            "summary",
            json!({"data":{"id":"record-7","type":"calendar_event","attributes":{"summary":"Dentist","all_day":false}}}),
        ),
        Mutation::CategoryCreate
        | Mutation::CategoryFindOrCreate
        | Mutation::CategoryUpdate
        | Mutation::CategoryDelete => (
            "category",
            "label",
            json!({"data":{"id":"record-7","type":"category","attributes":{"label":"Dentist","color":"#1E88E5"}}}),
        ),
        Mutation::ListCreate | Mutation::ListUpdate | Mutation::ListDelete => (
            "list",
            "label",
            json!({"data":{"id":"record-7","type":"list","attributes":{"label":"Dentist","kind":"shopping"}}}),
        ),
        _ => (
            "item",
            "label",
            json!({"data":{"id":"record-7","type":"list_item","attributes":{"label":"Dentist","status":"completed"}}}),
        ),
    }
}
fn write_response(status: u16, body: &[u8]) -> Response {
    Response {
        status,
        headers: vec![],
        body: body.to_vec(),
    }
}

/// The fixed write request: three read headers plus `content-type`, never a credential.
fn assert_write_request(request: &Request, method: &str, uri: &str, body: &[u8]) {
    assert_eq!(request.method, method);
    assert_eq!(request.uri, uri);
    assert_eq!(
        request.body,
        body,
        "{}",
        String::from_utf8_lossy(&request.body)
    );
    assert_eq!(
        request.headers,
        vec![
            Header::text("accept", ACCEPT_JSON).unwrap(),
            Header::text("user-agent", USER_AGENT).unwrap(),
            Header::text("skylight-api-version", SKYLIGHT_API_VERSION).unwrap(),
            Header::text("content-type", "application/json").unwrap(),
        ]
    );
    for denied in ["authorization", "cookie"] {
        assert!(
            !request
                .headers
                .iter()
                .any(|header| header.name.eq_ignore_ascii_case(denied)),
            "guest set denied header {denied}"
        );
    }
}

#[test]
fn each_event_write_sends_one_request_with_its_exact_method_uri_headers_and_json_body() {
    let base = "https://app.ourskylight.com/api/frames/frame-test/calendar_events";
    let cases: [(Mutation, &str, String, &[u8]); 3] = [
        (
            Mutation::EventCreate,
            "POST",
            base.to_owned(),
            br#"{"summary":"Dentist","starts_at":"2028-03-11T09:00:00-05:00","ends_at":"2028-03-11T10:00:00-05:00","all_day":false,"timezone":"America/New_York","category_ids":["category-1","category-2"],"rrule":["RRULE:FREQ=WEEKLY;COUNT=2"]}"#,
        ),
        (
            Mutation::EventUpdate,
            "PUT",
            format!("{base}/event-7"),
            br#"{"summary":"Dentist moved","all_day":true}"#,
        ),
        // DELETE still names its media type, with an empty body.
        (Mutation::EventDelete, "DELETE", format!("{base}/event-7"), b""),
    ];
    for (write, method, uri, body) in cases {
        let calls = std::cell::Cell::new(0);
        let output = write
            .invoke(write_input(write), |request| {
                calls.set(calls.get() + 1);
                assert_write_request(&request, method, &uri, body);
                Ok(write_response(204, b""))
            })
            .unwrap();
        assert_eq!(calls.get(), 1, "{write:?} must send exactly once");
        assert_eq!(output, json!({"ok": true}));
    }
    // A minimal create sends only its summary: absent optional fields are omitted, never null.
    Mutation::EventCreate
        .invoke(
            json!({"frameId":"frame-test","summary":"Dentist"}),
            |request| {
                assert_eq!(request.body, br#"{"summary":"Dentist"}"#);
                Ok(write_response(204, b""))
            },
        )
        .unwrap();
}

#[test]
fn write_2xx_responses_project_the_record_or_succeed_without_one_and_never_invite_a_retry() {
    for write in WRITES {
        let (key, text, record) = write_record(write);
        let record = record.to_string();
        for status in [200, 201] {
            let output = write
                .invoke(write_input(write), |_| {
                    Ok(write_response(status, record.as_bytes()))
                })
                .unwrap();
            assert_eq!(output[key]["id"], "record-7", "{write:?} {status}");
            assert_eq!(output[key][text], "Dentist");
            assert_eq!(output["coverage"], "bounded-response");
            assert!(output.get("ok").is_none());
        }
    }
    let record = write_record(Mutation::EventCreate).2.to_string();
    for write in WRITES {
        let output = write
            .invoke(write_input(write), |_| Ok(write_response(204, b"")))
            .unwrap();
        assert_eq!(output, json!({"ok": true}), "{write:?}");
        // The write applied: an unprojectable or oversized record is a success without a
        // record, never an error a caller would retry.
        let oversized = vec![b' '; MAX_RESPONSE_BODY_BYTES + 1];
        for status in [200, 201] {
            for body in [
                b"".as_slice(),
                b"{}",
                br#"{"data":null}"#,
                b"not json",
                &oversized,
            ] {
                let output = write
                    .invoke(write_input(write), |_| Ok(write_response(status, body)))
                    .unwrap();
                assert_eq!(
                    output,
                    json!({"ok": true, "recordProjected": false}),
                    "{write:?} {status}"
                );
            }
        }
        for (status, code) in [
            (202, "unexpected-status"),
            (403, "forbidden"),
            (404, "not-found"),
            (422, "unexpected-status"),
        ] {
            let error = write
                .invoke(write_input(write), |_| {
                    Ok(write_response(status, record.as_bytes()))
                })
                .unwrap_err();
            assert_eq!(error.code(), code, "{write:?} {status}");
        }
    }
    // Reads keep their 200-only contract: a write status is not a read success.
    for status in [201, 204] {
        let error = Read::Lists
            .invoke(input(Read::Lists), |_| {
                Ok(write_response(status, br#"{"data":[]}"#))
            })
            .unwrap_err();
        assert_eq!(error.code(), "unexpected-status", "read {status}");
    }
}

#[test]
fn malformed_event_write_inputs_are_refused_before_any_http() {
    let long = "x".repeat(MAX_NAME_BYTES + 1);
    let seventeen: Vec<String> = (0..17).map(|n| format!("category-{n}")).collect();
    let mut cases: Vec<(Mutation, &str, Value)> = Vec::new();
    for write in WRITES {
        for field in [
            "url",
            "endpoint",
            "host",
            "path",
            "query",
            "headers",
            "authorization",
            "token",
            "body",
            "method",
            "extra",
            "json",
            "include",
            "id",
        ] {
            cases.push((write, field, json!("secret-sentinel")));
        }
        for value in ["/", "%2F", "?", "#", "..", "a/b", "x\r\n", "", "é", "x.y"] {
            for selector in write_required(write)
                .iter()
                .filter(|field| field.ends_with("Id"))
            {
                cases.push((write, selector, json!(value)));
            }
        }
        cases.push((write, "frameId", Value::Null));
        cases.push((write, "frameId", json!(7)));
    }
    for write in [Mutation::EventCreate, Mutation::EventUpdate] {
        for (field, value) in [
            ("summary", json!("")),
            ("summary", json!("   ")),
            ("summary", json!(long)),
            ("summary", json!("line\nbreak")),
            ("summary", json!("tab\there")),
            ("summary", Value::Null),
            ("summary", json!(["Dentist"])),
            ("startsAt", json!("2028-02-30")),
            ("startsAt", json!("2028-03-11 09:00")),
            ("startsAt", json!("2028-03-11T24:00")),
            ("startsAt", json!("2028-03-11T09:60")),
            ("startsAt", json!("2028-03-11T09:00:00.")),
            ("startsAt", json!("2028-03-11T09:00:00+5")),
            ("startsAt", json!("2028-03-11T09:00:00Zjunk")),
            ("startsAt", json!("2028-03-1é")),
            (
                "startsAt",
                json!(format!("2028-03-11T09:00:00.{}", "1".repeat(60))),
            ),
            ("endsAt", json!("tomorrow")),
            ("endsAt", Value::Null),
            ("allDay", json!("true")),
            ("allDay", Value::Null),
            ("timezone", json!("")),
            ("timezone", json!("America//New_York")),
            ("timezone", json!("America/New York")),
            ("timezone", json!("x".repeat(129))),
        ] {
            cases.push((write, field, value));
        }
    }
    for (field, value) in [
        ("categoryIds", json!([])),
        ("categoryIds", json!("category-1")),
        ("categoryIds", json!(["category-1", "category-1"])),
        ("categoryIds", json!(["../secret-sentinel"])),
        ("categoryIds", json!([7])),
        ("categoryIds", json!(seventeen)),
        ("rrule", json!([])),
        ("rrule", json!([""])),
        ("rrule", json!(["RRULE:FREQ=DAILY\r\nX-SENTINEL"])),
        ("rrule", json!([long])),
        ("rrule", json!(vec!["RRULE:FREQ=DAILY"; 17])),
    ] {
        cases.push((Mutation::EventCreate, field, value));
    }
    // Update accepts neither categories nor recurrence (INVENTORY row 8).
    cases.push((Mutation::EventUpdate, "categoryIds", json!(["category-1"])));
    cases.push((Mutation::EventUpdate, "rrule", json!(["RRULE:FREQ=DAILY"])));
    cases.push((Mutation::EventDelete, "summary", json!("Dentist")));
    for (write, field, value) in cases {
        let mut data = write_input(write);
        data[field] = value.clone();
        let error = write
            .invoke(data, |_| panic!("{write:?} sent invalid {field}={value}"))
            .unwrap_err();
        assert_eq!(error.code(), "invalid-input", "{write:?} {field}={value}");
    }
    for write in WRITES {
        for &field in write_required(write) {
            let mut data = write_input(write);
            data.as_object_mut().unwrap().remove(field);
            assert!(write.uri(&data).is_err(), "{write:?} without {field}");
        }
        for bad in [Value::Null, json!([]), json!({}), json!("x")] {
            assert!(write.uri(&bad).is_err());
        }
    }
    // An update must change something.
    assert!(
        Mutation::EventUpdate
            .uri(&json!({"frameId":"frame-test","eventId":"event-7"}))
            .is_err()
    );
    // Accepted edges: bare dates, UTC, fractions, 16 categories and rules, 256-byte text.
    let sixteen: Vec<String> = (0..16).map(|n| format!("category-{n}")).collect();
    for (field, value) in [
        ("startsAt", json!("2028-02-29")),
        ("startsAt", json!("2028-03-11T09:00Z")),
        ("startsAt", json!("2028-03-11T23:59:59.123456789+14:00")),
        ("categoryIds", json!(sixteen)),
        ("rrule", json!(vec!["EXDATE:20280318T090000"; 16])),
        ("summary", json!("é".repeat(MAX_NAME_BYTES / 2))),
    ] {
        let mut data = create_input();
        data[field] = value.clone();
        assert!(Mutation::EventCreate.body(&data).is_ok(), "{field}={value}");
    }
}

#[test]
fn event_write_commands_propose_exact_inputs_and_refuse_invalid_or_repeated_flags() {
    let cases = [
        (
            vec![
                "event-add",
                "--frame",
                "frame-test",
                "--summary",
                "Dentist",
                "--starts-at",
                "2028-03-11T09:00:00-05:00",
                "--ends-at",
                "2028-03-11T10:00:00-05:00",
                "--all-day",
                "false",
                "--tz",
                "America/New_York",
                "--category",
                "category-1",
                "--rrule",
                "RRULE:FREQ=WEEKLY;COUNT=2",
                "--category",
                "category-2",
            ],
            Mutation::EventCreate,
            create_input(),
        ),
        (
            vec!["event-add", "--frame", "frame-test", "--summary", "Dentist"],
            Mutation::EventCreate,
            json!({"frameId":"frame-test","summary":"Dentist"}),
        ),
        (
            vec![
                "event-update",
                "--frame",
                "frame-test",
                "--event",
                "event-7",
                "--summary",
                "Dentist moved",
                "--all-day",
                "true",
            ],
            Mutation::EventUpdate,
            write_input(Mutation::EventUpdate),
        ),
        (
            vec![
                "event-delete",
                "--event",
                "event-7",
                "--frame",
                "frame-test",
            ],
            Mutation::EventDelete,
            write_input(Mutation::EventDelete),
        ),
    ];
    for (words, write, expected) in cases {
        let argv: Vec<_> = words.into_iter().map(str::to_owned).collect();
        let dekopon_provider_sdk::CommandRunOutcome::Proposed {
            capability, input, ..
        } = provider::command::<SkylightPrivate>(&argv, true)
        else {
            panic!("expected proposal for {argv:?}");
        };
        assert_eq!(capability.as_str(), write.capability());
        assert_eq!(input, expected);
    }
    for words in [
        vec!["event-add", "--frame", "frame-test"],
        vec![
            "event-add",
            "--frame",
            "x",
            "--summary",
            "a",
            "--summary",
            "b",
        ],
        vec![
            "event-add",
            "--frame",
            "x",
            "--frame",
            "y",
            "--summary",
            "a",
        ],
        vec![
            "event-add",
            "--frame",
            "x",
            "--summary",
            "a",
            "--category",
            "../secret-sentinel",
        ],
        vec![
            "event-add",
            "--frame",
            "x",
            "--summary",
            "a",
            "--starts-at",
            "secret-sentinel",
        ],
        vec![
            "event-add",
            "--frame",
            "x",
            "--summary",
            "a",
            "--json",
            "{}",
        ],
        vec![
            "event-add",
            "--frame",
            "x",
            "--summary",
            "a",
            "--all-day",
            "yes",
        ],
        vec!["event-update", "--frame", "x", "--event", "y"],
        vec![
            "event-update",
            "--frame",
            "x",
            "--event",
            "y",
            "--category",
            "c",
        ],
        vec![
            "event-update",
            "--frame",
            "x",
            "--event",
            "y",
            "--tz",
            "a",
            "--tz",
            "b",
        ],
        vec!["event-delete", "--frame", "x"],
        vec![
            "event-delete",
            "--frame",
            "x",
            "--event",
            "../secret-sentinel",
        ],
        vec![
            "event-delete",
            "--frame",
            "x",
            "--event",
            "y",
            "--summary",
            "z",
        ],
        vec!["event-delete", "x", "y"],
    ] {
        let argv: Vec<_> = words.into_iter().map(str::to_owned).collect();
        let outcome = provider::command::<SkylightPrivate>(&argv, false);
        assert!(
            !matches!(
                outcome,
                dekopon_provider_sdk::CommandRunOutcome::Proposed { .. }
            ),
            "invalid argv accepted: {argv:?}"
        );
        assert!(!format!("{outcome:?}").contains("secret-sentinel"));
    }
}

#[test]
fn each_list_item_write_sends_one_request_with_its_exact_method_uri_headers_and_json_body() {
    let base = "https://app.ourskylight.com/api/frames/frame-test/lists/list-test/list_items";
    let cases: [(Mutation, Value, &str, String, &[u8]); 6] = [
        (
            Mutation::ItemCreate,
            write_input(Mutation::ItemCreate),
            "POST",
            base.to_owned(),
            br#"{"label":"Milk","section":"Dairy"}"#,
        ),
        (
            Mutation::ItemCreate,
            json!({"frameId":"frame-test", "listId":"list-test", "label":"Milk"}),
            "POST",
            base.to_owned(),
            br#"{"label":"Milk"}"#,
        ),
        (
            Mutation::ItemUpdate,
            write_input(Mutation::ItemUpdate),
            "PUT",
            format!("{base}/item-7"),
            br#"{"label":"Oat milk","status":"pending","position":3}"#,
        ),
        // Completing without a status completes; `pending` reopens.
        (
            Mutation::ItemComplete,
            write_input(Mutation::ItemComplete),
            "PUT",
            format!("{base}/item-7"),
            br#"{"status":"completed"}"#,
        ),
        (
            Mutation::ItemComplete,
            json!({"frameId":"frame-test", "listId":"list-test", "itemId":"item-7", "status":"pending"}),
            "PUT",
            format!("{base}/item-7"),
            br#"{"status":"pending"}"#,
        ),
        (
            Mutation::ItemDelete,
            write_input(Mutation::ItemDelete),
            "DELETE",
            format!("{base}/item-7"),
            b"",
        ),
    ];
    for (write, input, method, uri, body) in cases {
        let calls = std::cell::Cell::new(0);
        write
            .invoke(input, |request| {
                calls.set(calls.get() + 1);
                assert_write_request(&request, method, &uri, body);
                Ok(write_response(204, b""))
            })
            .unwrap();
        assert_eq!(calls.get(), 1, "{write:?} must send exactly once");
    }
}

#[test]
fn malformed_list_item_write_inputs_are_refused_before_any_http() {
    let long = "x".repeat(MAX_NAME_BYTES + 1);
    let mut cases: Vec<(Mutation, &str, Value)> = Vec::new();
    for (field, value) in [
        ("label", json!("")),
        ("label", json!("   ")),
        ("label", json!(long)),
        ("label", json!("line\nbreak")),
        ("label", Value::Null),
        ("label", json!(["Milk"])),
        ("section", json!("")),
        ("section", json!(long)),
        ("section", json!("tab\there")),
        ("section", Value::Null),
        ("status", json!("completed")),
        ("position", json!(1)),
    ] {
        cases.push((Mutation::ItemCreate, field, value));
    }
    for write in [Mutation::ItemUpdate, Mutation::ItemComplete] {
        for value in [
            json!("done"),
            json!("COMPLETED"),
            json!("complete"),
            json!(""),
            json!(true),
            Value::Null,
        ] {
            cases.push((write, "status", value));
        }
    }
    for (field, value) in [
        ("label", json!("")),
        ("label", json!(long)),
        ("position", json!(-1)),
        ("position", json!(1.5)),
        ("position", json!(1_000_001)),
        ("position", json!("3")),
        ("position", Value::Null),
        ("section", json!("Dairy")),
    ] {
        cases.push((Mutation::ItemUpdate, field, value));
    }
    cases.push((Mutation::ItemComplete, "label", json!("Milk")));
    cases.push((Mutation::ItemComplete, "position", json!(3)));
    cases.push((Mutation::ItemDelete, "status", json!("completed")));
    for (write, field, value) in cases {
        let mut data = write_input(write);
        data[field] = value.clone();
        let error = write
            .invoke(data, |_| panic!("{write:?} sent invalid {field}={value}"))
            .unwrap_err();
        assert_eq!(error.code(), "invalid-input", "{write:?} {field}={value}");
    }
    // An item update must change something; position 0 and 1,000,000 are the accepted bounds.
    let selectors = json!({"frameId":"frame-test","listId":"list-test","itemId":"item-7"});
    assert!(Mutation::ItemUpdate.uri(&selectors).is_err());
    for position in [0, 1_000_000] {
        let mut data = selectors.clone();
        data["position"] = json!(position);
        assert!(Mutation::ItemUpdate.body(&data).is_ok(), "{position}");
    }
}

#[test]
fn list_item_write_commands_propose_exact_inputs_and_refuse_invalid_or_repeated_flags() {
    let cases = [
        (
            vec![
                "list-add",
                "--frame",
                "frame-test",
                "--list",
                "list-test",
                "--label",
                "Milk",
                "--section",
                "Dairy",
            ],
            Mutation::ItemCreate,
            write_input(Mutation::ItemCreate),
        ),
        (
            vec![
                "list-item-update",
                "--frame",
                "frame-test",
                "--list",
                "list-test",
                "--item",
                "item-7",
                "--label",
                "Oat milk",
                "--status",
                "pending",
                "--position",
                "3",
            ],
            Mutation::ItemUpdate,
            write_input(Mutation::ItemUpdate),
        ),
        (
            vec![
                "list-item-complete",
                "--frame",
                "frame-test",
                "--list",
                "list-test",
                "--item",
                "item-7",
            ],
            Mutation::ItemComplete,
            write_input(Mutation::ItemComplete),
        ),
        (
            vec![
                "list-item-delete",
                "--item",
                "item-7",
                "--list",
                "list-test",
                "--frame",
                "frame-test",
            ],
            Mutation::ItemDelete,
            write_input(Mutation::ItemDelete),
        ),
    ];
    for (words, write, expected) in cases {
        let argv: Vec<_> = words.into_iter().map(str::to_owned).collect();
        let dekopon_provider_sdk::CommandRunOutcome::Proposed {
            capability, input, ..
        } = provider::command::<SkylightPrivate>(&argv, true)
        else {
            panic!("expected proposal for {argv:?}");
        };
        assert_eq!(capability.as_str(), write.capability());
        assert_eq!(input, expected);
    }
    for words in [
        vec!["list-add", "--frame", "x", "--list", "y"],
        vec![
            "list-add", "--frame", "x", "--list", "y", "--label", "a", "--label", "b",
        ],
        vec![
            "list-add",
            "--frame",
            "x",
            "--list",
            "../secret-sentinel",
            "--label",
            "a",
        ],
        vec![
            "list-add",
            "--frame",
            "x",
            "--list",
            "y",
            "--label",
            "a",
            "--status",
            "completed",
        ],
        vec![
            "list-item-update",
            "--frame",
            "x",
            "--list",
            "y",
            "--item",
            "z",
        ],
        vec![
            "list-item-update",
            "--frame",
            "x",
            "--list",
            "y",
            "--item",
            "z",
            "--status",
            "done",
        ],
        vec![
            "list-item-update",
            "--frame",
            "x",
            "--list",
            "y",
            "--item",
            "z",
            "--position",
            "-1",
        ],
        vec![
            "list-item-complete",
            "--frame",
            "x",
            "--list",
            "y",
            "--item",
            "z",
            "--status",
            "secret-sentinel",
        ],
        vec!["list-item-complete", "--frame", "x", "--list", "y"],
        vec![
            "list-item-delete",
            "--frame",
            "x",
            "--list",
            "y",
            "--item",
            "z",
            "--label",
            "a",
        ],
    ] {
        let argv: Vec<_> = words.into_iter().map(str::to_owned).collect();
        let outcome = provider::command::<SkylightPrivate>(&argv, false);
        assert!(
            !matches!(
                outcome,
                dekopon_provider_sdk::CommandRunOutcome::Proposed { .. }
            ),
            "invalid argv accepted: {argv:?}"
        );
        assert!(!format!("{outcome:?}").contains("secret-sentinel"));
    }
}

#[test]
fn each_list_write_sends_one_request_with_its_exact_method_uri_headers_and_json_body() {
    let base = "https://app.ourskylight.com/api/frames/frame-test/lists";
    let cases: [(Mutation, Value, &str, String, &[u8]); 4] = [
        (
            Mutation::ListCreate,
            write_input(Mutation::ListCreate),
            "POST",
            base.to_owned(),
            br##"{"label":"Groceries","color":"#2E7D32","kind":"shopping"}"##,
        ),
        (
            Mutation::ListCreate,
            json!({"frameId":"frame-test", "label":"Groceries"}),
            "POST",
            base.to_owned(),
            br#"{"label":"Groceries"}"#,
        ),
        (
            Mutation::ListUpdate,
            write_input(Mutation::ListUpdate),
            "PUT",
            format!("{base}/list-test"),
            br#"{"label":"Chores","kind":"to_do"}"#,
        ),
        (
            Mutation::ListDelete,
            write_input(Mutation::ListDelete),
            "DELETE",
            format!("{base}/list-test"),
            b"",
        ),
    ];
    for (write, input, method, uri, body) in cases {
        let calls = std::cell::Cell::new(0);
        write
            .invoke(input, |request| {
                calls.set(calls.get() + 1);
                assert_write_request(&request, method, &uri, body);
                Ok(write_response(204, b""))
            })
            .unwrap();
        assert_eq!(calls.get(), 1, "{write:?} must send exactly once");
    }
}

#[test]
fn malformed_list_write_inputs_are_refused_before_any_http() {
    let long = "x".repeat(MAX_NAME_BYTES + 1);
    let mut cases: Vec<(Mutation, &str, Value)> = Vec::new();
    for write in [Mutation::ListCreate, Mutation::ListUpdate] {
        for (field, value) in [
            ("label", json!("")),
            ("label", json!("   ")),
            ("label", json!(long)),
            ("label", json!("line\nbreak")),
            ("label", Value::Null),
            ("color", json!("")),
            ("color", json!("x".repeat(33))),
            ("color", json!("rgb(1, 2, 3)")),
            ("color", json!("#2E7D32;")),
            ("color", Value::Null),
            ("kind", json!("grocery")),
            ("kind", json!("SHOPPING")),
            ("kind", json!("")),
            ("kind", Value::Null),
            ("hideFromFrame", json!(true)),
            ("hideOnDevice", json!(true)),
        ] {
            cases.push((write, field, value));
        }
    }
    cases.push((Mutation::ListDelete, "label", json!("Groceries")));
    for (write, field, value) in cases {
        let mut data = write_input(write);
        data[field] = value.clone();
        let error = write
            .invoke(data, |_| panic!("{write:?} sent invalid {field}={value}"))
            .unwrap_err();
        assert_eq!(error.code(), "invalid-input", "{write:?} {field}={value}");
    }
    // A list update must change something; a palette name and a 32-byte color are accepted.
    let selectors = json!({"frameId":"frame-test","listId":"list-test"});
    assert!(Mutation::ListUpdate.uri(&selectors).is_err());
    for color in ["blue", "x".repeat(32).as_str()] {
        let mut data = selectors.clone();
        data["color"] = json!(color);
        assert!(Mutation::ListUpdate.body(&data).is_ok(), "{color}");
    }
}

#[test]
fn list_write_commands_propose_exact_inputs_and_refuse_invalid_or_repeated_flags() {
    let cases = [
        (
            vec![
                "list-create",
                "--frame",
                "frame-test",
                "--label",
                "Groceries",
                "--color",
                "#2E7D32",
                "--kind",
                "shopping",
            ],
            Mutation::ListCreate,
            write_input(Mutation::ListCreate),
        ),
        (
            vec![
                "list-update",
                "--frame",
                "frame-test",
                "--list",
                "list-test",
                "--label",
                "Chores",
                "--kind",
                "to_do",
            ],
            Mutation::ListUpdate,
            write_input(Mutation::ListUpdate),
        ),
        (
            vec![
                "list-delete",
                "--list",
                "list-test",
                "--frame",
                "frame-test",
            ],
            Mutation::ListDelete,
            write_input(Mutation::ListDelete),
        ),
    ];
    for (words, write, expected) in cases {
        let argv: Vec<_> = words.into_iter().map(str::to_owned).collect();
        let dekopon_provider_sdk::CommandRunOutcome::Proposed {
            capability, input, ..
        } = provider::command::<SkylightPrivate>(&argv, true)
        else {
            panic!("expected proposal for {argv:?}");
        };
        assert_eq!(capability.as_str(), write.capability());
        assert_eq!(input, expected);
    }
    for words in [
        vec!["list-create", "--frame", "x"],
        vec![
            "list-create",
            "--frame",
            "x",
            "--label",
            "a",
            "--label",
            "b",
        ],
        vec![
            "list-create",
            "--frame",
            "x",
            "--label",
            "a",
            "--kind",
            "secret-sentinel",
        ],
        vec![
            "list-create",
            "--frame",
            "x",
            "--label",
            "a",
            "--hide-from-frame",
        ],
        vec!["list-update", "--frame", "x", "--list", "y"],
        vec![
            "list-update",
            "--frame",
            "x",
            "--list",
            "../secret-sentinel",
            "--label",
            "a",
        ],
        vec!["list-delete", "--frame", "x"],
        vec!["list-delete", "--frame", "x", "--list", "y", "--label", "a"],
    ] {
        let argv: Vec<_> = words.into_iter().map(str::to_owned).collect();
        let outcome = provider::command::<SkylightPrivate>(&argv, false);
        assert!(
            !matches!(
                outcome,
                dekopon_provider_sdk::CommandRunOutcome::Proposed { .. }
            ),
            "invalid argv accepted: {argv:?}"
        );
        assert!(!format!("{outcome:?}").contains("secret-sentinel"));
    }
}

#[test]
fn each_category_write_sends_one_request_with_its_exact_method_uri_headers_and_json_body() {
    let base = "https://app.ourskylight.com/api/frames/frame-test/categories";
    let cases: [(Mutation, Value, &str, String, &[u8]); 5] = [
        (
            Mutation::CategoryCreate,
            write_input(Mutation::CategoryCreate),
            "POST",
            base.to_owned(),
            br##"{"label":"Swim","color":"#1E88E5"}"##,
        ),
        (
            Mutation::CategoryFindOrCreate,
            write_input(Mutation::CategoryFindOrCreate),
            "POST",
            format!("{base}/find_or_create"),
            br##"{"label":"Swim","color":"#1E88E5"}"##,
        ),
        (
            Mutation::CategoryUpdate,
            write_input(Mutation::CategoryUpdate),
            "PATCH",
            format!("{base}/category-7"),
            br#"{"label":"Swimming"}"#,
        ),
        // The one write with a query: the reassignment target, a bounded selector.
        (
            Mutation::CategoryDelete,
            write_input(Mutation::CategoryDelete),
            "DELETE",
            format!("{base}/category-7?reassign_to_id=category-8"),
            b"",
        ),
        (
            Mutation::CategoryDelete,
            json!({"frameId":"frame-test", "categoryId":"category-7"}),
            "DELETE",
            format!("{base}/category-7"),
            b"",
        ),
    ];
    for (write, input, method, uri, body) in cases {
        let calls = std::cell::Cell::new(0);
        write
            .invoke(input, |request| {
                calls.set(calls.get() + 1);
                assert_write_request(&request, method, &uri, body);
                Ok(write_response(204, b""))
            })
            .unwrap();
        assert_eq!(calls.get(), 1, "{write:?} must send exactly once");
    }
}

#[test]
fn malformed_category_write_inputs_are_refused_before_any_http() {
    let long = "x".repeat(MAX_NAME_BYTES + 1);
    let mut cases: Vec<(Mutation, &str, Value)> = Vec::new();
    for write in [
        Mutation::CategoryCreate,
        Mutation::CategoryFindOrCreate,
        Mutation::CategoryUpdate,
    ] {
        for (field, value) in [
            ("label", json!("")),
            ("label", json!("   ")),
            ("label", json!(long)),
            ("label", json!("tab\there")),
            ("label", Value::Null),
            ("color", json!("")),
            ("color", json!("x".repeat(33))),
            ("color", json!("rgb(1, 2, 3)")),
            ("color", Value::Null),
            ("reassignToId", json!("category-8")),
        ] {
            cases.push((write, field, value));
        }
    }
    for value in ["/", "%2F", "?", "#", "..", "a/b", "x&y=z", "", "é", "x.y"] {
        cases.push((Mutation::CategoryDelete, "reassignToId", json!(value)));
    }
    cases.push((Mutation::CategoryDelete, "reassignToId", Value::Null));
    cases.push((Mutation::CategoryDelete, "label", json!("Swim")));
    for (write, field, value) in cases {
        let mut data = write_input(write);
        data[field] = value.clone();
        let error = write
            .invoke(data, |_| panic!("{write:?} sent invalid {field}={value}"))
            .unwrap_err();
        assert_eq!(error.code(), "invalid-input", "{write:?} {field}={value}");
    }
    // A category update must change something.
    assert!(
        Mutation::CategoryUpdate
            .uri(&json!({"frameId":"frame-test","categoryId":"category-7"}))
            .is_err()
    );
}

#[test]
fn category_write_commands_propose_exact_inputs_and_refuse_invalid_or_repeated_flags() {
    let cases = [
        (
            vec![
                "category-add",
                "--frame",
                "frame-test",
                "--label",
                "Swim",
                "--color",
                "#1E88E5",
            ],
            Mutation::CategoryCreate,
            write_input(Mutation::CategoryCreate),
        ),
        (
            vec![
                "category-find-or-create",
                "--frame",
                "frame-test",
                "--label",
                "Swim",
                "--color",
                "#1E88E5",
            ],
            Mutation::CategoryFindOrCreate,
            write_input(Mutation::CategoryFindOrCreate),
        ),
        (
            vec![
                "category-update",
                "--frame",
                "frame-test",
                "--category",
                "category-7",
                "--label",
                "Swimming",
            ],
            Mutation::CategoryUpdate,
            write_input(Mutation::CategoryUpdate),
        ),
        (
            vec![
                "category-delete",
                "--frame",
                "frame-test",
                "--category",
                "category-7",
                "--reassign-to",
                "category-8",
            ],
            Mutation::CategoryDelete,
            write_input(Mutation::CategoryDelete),
        ),
    ];
    for (words, write, expected) in cases {
        let argv: Vec<_> = words.into_iter().map(str::to_owned).collect();
        let dekopon_provider_sdk::CommandRunOutcome::Proposed {
            capability, input, ..
        } = provider::command::<SkylightPrivate>(&argv, true)
        else {
            panic!("expected proposal for {argv:?}");
        };
        assert_eq!(capability.as_str(), write.capability());
        assert_eq!(input, expected);
    }
    for words in [
        vec!["category-add", "--frame", "x", "--label", "a"],
        vec![
            "category-add",
            "--frame",
            "x",
            "--label",
            "a",
            "--color",
            "b",
            "--color",
            "c",
        ],
        vec!["category-find-or-create", "--frame", "x", "--color", "b"],
        vec!["category-update", "--frame", "x", "--category", "y"],
        vec![
            "category-delete",
            "--frame",
            "x",
            "--category",
            "y",
            "--reassign-to",
            "../secret-sentinel",
        ],
        vec![
            "category-delete",
            "--frame",
            "x",
            "--category",
            "y",
            "--label",
            "a",
        ],
    ] {
        let argv: Vec<_> = words.into_iter().map(str::to_owned).collect();
        let outcome = provider::command::<SkylightPrivate>(&argv, false);
        assert!(
            !matches!(
                outcome,
                dekopon_provider_sdk::CommandRunOutcome::Proposed { .. }
            ),
            "invalid argv accepted: {argv:?}"
        );
        assert!(!format!("{outcome:?}").contains("secret-sentinel"));
    }
}

#[test]
fn media_reads_are_fixed_gets_and_cli_matches_every_manifest_entry() {
    let cases = [
        (Read::Messages, "messages", "/messages"),
        (Read::Message, "message", "/messages/message-test"),
        (
            Read::MessageLikes,
            "message-likes",
            "/messages/message-test/all_likes",
        ),
        (
            Read::MessageComments,
            "message-comments",
            "/messages/message-test/comments",
        ),
        (Read::Albums, "albums", "/albums"),
        (
            Read::AlbumMessages,
            "album-messages",
            "/albums/album-test/messages",
        ),
        (
            Read::AlbumMessageIds,
            "album-message-ids",
            "/albums/album-test/messages/all_ids",
        ),
        (Read::MonthInReview, "month-in-review", "/month_in_review"),
        (
            Read::MonthInReviews,
            "month-in-reviews",
            "/month_in_reviews",
        ),
    ];
    assert_eq!(cases.len(), MEDIA_READS.len());
    for (read, verb, path) in cases {
        // No query: page and page_token are never sent, so pagination is never followed.
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
        let dekopon_provider_sdk::CommandRunOutcome::Proposed {
            capability,
            input: proposed,
            ..
        } = provider::command::<SkylightPrivate>(&argv, true)
        else {
            panic!("expected proposal for {argv:?}");
        };
        assert_eq!(capability.as_str(), read.capability());
        assert_eq!(proposed, input(read));
        let manifest = read.manifest();
        assert_eq!(manifest.input_schema["additionalProperties"], false);
        let actual = provider::manifest::<SkylightPrivate>().unwrap();
        let declared = actual
            .capabilities
            .iter()
            .find(|capability| capability.id == manifest.id)
            .unwrap();
        assert_eq!(declared.description, manifest.description);
        assert_eq!(declared.input_schema, manifest.input_schema);
        assert_eq!(declared.effect, EffectKind::ReadOnly);
    }
    for read in MEDIA_READS {
        for field in ["page", "page_token", "pageToken", "url", "include", "query"] {
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
            for (_, field) in read.fields() {
                let mut data = input(read);
                data[*field] = json!(value);
                assert!(read.uri(&data).is_err(), "{read:?} {field}={value}");
            }
        }
    }
}

#[test]
fn media_projections_keep_ids_and_text_and_never_forward_media_urls() {
    let record = |id: &str| {
        json!({
            "id": id,
            "type": "message",
            "attributes": {
                "caption": "Synthetic beach day",
                "created_at": "2028-03-11T09:00:00Z",
                "asset_url": "https://media.example.invalid/secret-sentinel.jpg",
                "thumbnail_url": "https://media.example.invalid/thumb-sentinel.jpg",
                "url": "https://media.example.invalid/url-sentinel",
                "file_name": "file-sentinel.jpg",
                "width": 4032
            },
            "relationships": {"category": {"data": {"type": "category", "id": "category-7"}}},
            "links": {"self": "https://media.example.invalid/link-sentinel"}
        })
    };
    let many = json!({
        "data": [record("message-2"), record("message-1")],
        "included": [{"type": "user", "id": "user-1", "attributes": {"avatar_url": "https://media.example.invalid/avatar-sentinel", "email": "email-sentinel@example.invalid"}}],
        "meta": {"next_page_token": "token-sentinel"},
        "links": {"next": "https://media.example.invalid/next-sentinel"}
    });
    for read in MEDIA_READS {
        let single = matches!(read, Read::Message | Read::MonthInReview);
        let body = if single {
            json!({"data": record("message-1")})
        } else {
            many.clone()
        };
        let output = invoke(read, body).unwrap();
        let rendered = output.to_string();
        for sentinel in [
            "secret-sentinel",
            "thumb-sentinel",
            "url-sentinel",
            "file-sentinel",
            "link-sentinel",
            "avatar-sentinel",
            "email-sentinel",
            "token-sentinel",
            "next-sentinel",
            "4032",
        ] {
            assert!(!rendered.contains(sentinel), "{read:?} leaked {sentinel}");
        }
        assert_eq!(output["coverage"], "bounded-response");
        assert_eq!(output["upstreamCompleteness"], "unknown");
        let first = output
            .as_object()
            .unwrap()
            .iter()
            .find_map(|(_, value)| match value {
                Value::Array(records)
                    if records.first().is_some_and(|r| r.get("caption").is_some()) =>
                {
                    records.first().cloned()
                }
                Value::Object(record) if record.contains_key("caption") => Some(value.clone()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{read:?} projected no record: {output}"));
        assert_eq!(first["id"], "message-1", "{read:?}: records are ID-sorted");
        assert_eq!(first["caption"], "Synthetic beach day");
        assert_eq!(first["createdAt"], "2028-03-11T09:00:00Z");
        assert_eq!(
            first["relationships"]["category"]["data"]["id"],
            "category-7"
        );
    }
    // A non-string caption is treated as absent rather than failing the whole read.
    let output = invoke(
        Read::Albums,
        json!({"data": [{"id": "album-1", "attributes": {"title": "Synthetic album", "caption": {"x": 1}}}]}),
    )
    .unwrap();
    assert_eq!(output["albums"][0]["title"], "Synthetic album");
    assert_eq!(output["albums"][0]["caption"], Value::Null);
    // A media read rejects a malformed envelope like every other read.
    for body in [json!({}), json!({"data": "x"}), json!([])] {
        assert_eq!(
            invoke(Read::Messages, body).unwrap_err().code(),
            "invalid-response"
        );
    }
}

#[test]
fn media_reads_project_included_resources_as_type_and_id_only() {
    let body = json!({
        "data": [{"id": "message-1", "type": "message", "attributes": {"caption": "Synthetic"}}],
        "included": [
            {"type": "category", "id": "category-7", "attributes": {"label": "category-label-sentinel", "color": "#123456", "linked_to_profile": true}},
            {"type": "avatar", "id": "avatar-7", "attributes": {"name": "avatar-name-sentinel", "kind": "avatar-kind-sentinel"}},
            {"type": "list_item", "id": "item-7", "attributes": {"label": "item-label-sentinel", "status": "pending", "section": "section-sentinel"}, "relationships": {"list": {"data": {"type": "list", "id": "list-7"}}}}
        ]
    });
    for read in MEDIA_READS {
        let single = matches!(read, Read::Message | Read::MonthInReview);
        let mut body = body.clone();
        if single {
            body["data"] = body["data"][0].clone();
        }
        let output = invoke(read, body).unwrap();
        let included = output["included"].as_array().unwrap();
        assert_eq!(included.len(), 3, "{read:?}");
        for record in included {
            let keys: Vec<_> = record
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect();
            assert_eq!(keys, ["id", "type"], "{read:?}: {record}");
        }
        let rendered = output.to_string();
        for sentinel in [
            "label-sentinel",
            "#123456",
            "avatar-name-sentinel",
            "avatar-kind-sentinel",
            "section-sentinel",
            "list-7",
        ] {
            assert!(!rendered.contains(sentinel), "{read:?} leaked {sentinel}");
        }
    }
    // Household reads keep their typed included attributes.
    let lists = invoke(
        Read::Lists,
        json!({"data": [], "included": [{"type": "category", "id": "category-7", "attributes": {"label": "Kept"}}]}),
    )
    .unwrap();
    assert_eq!(lists["included"][0]["label"], "Kept");
}

#[test]
fn truncated_media_text_marks_the_record_and_the_output_truncated() {
    let long = "x".repeat(MAX_NAME_BYTES + 1);
    for field in ["caption", "title", "body"] {
        let record = json!({"id": "message-1", "attributes": {field: long}});
        let many = invoke(Read::Messages, json!({"data": [record]})).unwrap();
        assert_eq!(many["messages"][0]["textTruncated"], true, "{field}");
        assert_eq!(many["messages"][0]["truncated"], true, "{field}");
        assert_eq!(many["truncated"], true, "{field}");
        assert!(
            many["messages"][0][field]
                .as_str()
                .unwrap()
                .ends_with(NAME_TRUNCATION_MARKER)
        );
        let one = invoke(Read::Message, json!({"data": record})).unwrap();
        assert_eq!(one["message"]["textTruncated"], true, "{field}");
        assert_eq!(one["message"]["truncated"], true, "{field}");
        assert_eq!(one["truncated"], true, "{field}");
    }
    let exact = invoke(
        Read::Messages,
        json!({"data": [{"id": "message-1", "attributes": {"caption": "x".repeat(MAX_NAME_BYTES)}}]}),
    )
    .unwrap();
    assert_eq!(exact["messages"][0]["truncated"], false);
    assert_eq!(exact["truncated"], false);
}
