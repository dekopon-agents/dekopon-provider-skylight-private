use serde_json::{Value, json};

pub struct Case {
    pub capability: &'static str,
    pub input: Value,
    pub path: &'static str,
    pub key: &'static str,
    pub body: Value,
    pub argv: Vec<&'static str>,
}

pub fn cases() -> Vec<Case> {
    vec![
        Case {
            capability: "skylight.private.tasks.list",
            input: json!({"frameId":"frame-test","after":"2028-03-11","before":"2028-03-11","includeLate":true,"includeUpForGrabs":false,"filter":"linked_to_profile"}),
            path: "/chores?after=2028-03-11&before=2028-03-11&include_late=true&include_up_for_grabs=false&filter=linked_to_profile",
            key: "tasks",
            body: json!({"data":[{"id":"sample","type":"chore","attributes":{"summary":"Synthetic task","status":"complete","completed_at":"opaque source time","start":"2028-03-11","recurrence_set":["RRULE:FREQ=DAILY"],"recurring":true},"relationships":{"category":{"data":{"type":"category","id":"category-test"}},"completed_category":{"data":null}}}],"included":[{"type":"category","id":"category-test","attributes":{"label":"Synthetic profile","linked_to_profile":true},"relationships":{"family_member":{"data":{"id":"member-test","type":"family_member"}}}}]}),
            argv: vec![
                "tasks",
                "--frame",
                "frame-test",
                "--after",
                "2028-03-11",
                "--before",
                "2028-03-11",
                "--include-late",
                "true",
                "--include-up-for-grabs",
                "false",
                "--filter",
                "linked_to_profile",
            ],
        },
        Case {
            capability: "skylight.private.categories.list",
            input: json!({"frameId":"frame-test"}),
            path: "/categories",
            key: "categories",
            body: json!({"data":[{"id":"sample","attributes":{"label":"Sample category"}}]}),
            argv: vec!["categories", "--frame", "frame-test"],
        },
        Case {
            capability: "skylight.private.calendar.events.list",
            input: json!({"frameId":"frame-test","dateMin":"2028-03-11","dateMax":"2028-03-13","timezone":"America/New_York","include":"categories,calendar_account,event_notification_setting"}),
            path: "/calendar_events?date_min=2028-03-11&date_max=2028-03-13&timezone=America%2FNew_York&include=categories%2Ccalendar_account%2Cevent_notification_setting",
            key: "events",
            body: json!({"data":[{"id":"sample","attributes":{"summary":"Possible trip","starts_at":"2028-03-10T23:00:00-05:00","ends_at":"2028-03-14T01:00:00-04:00","all_day":false}}]}),
            argv: vec![
                "events",
                "--frame",
                "frame-test",
                "--from",
                "2028-03-11",
                "--to",
                "2028-03-13",
                "--tz",
                "America/New_York",
                "--include",
                "categories,calendar_account,event_notification_setting",
            ],
        },
        Case {
            capability: "skylight.private.lists.list",
            input: json!({"frameId":"frame-test"}),
            path: "/lists",
            key: "lists",
            body: json!({"data":[{"id":"sample","attributes":{"label":"Sample list"}}]}),
            argv: vec!["lists", "--frame", "frame-test"],
        },
        Case {
            capability: "skylight.private.lists.read",
            input: json!({"frameId":"frame-test","listId":"list-test"}),
            path: "/lists/list-test",
            key: "list",
            body: json!({"data":{"id":"sample","attributes":{"label":"Sample list"}}}),
            argv: vec!["list-show", "--frame", "frame-test", "--list", "list-test"],
        },
        Case {
            capability: "skylight.private.list.items.list",
            input: json!({"frameId":"frame-test","listId":"list-test"}),
            path: "/lists/list-test/list_items",
            key: "items",
            body: json!({"data":[{"id":"sample","attributes":{"label":"Sample item","status":"unknown-status","section":null}}]}),
            argv: vec!["list-items", "--frame", "frame-test", "--list", "list-test"],
        },
    ]
}
