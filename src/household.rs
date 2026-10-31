//! Bounded source-backed household reads. Linkage is evidence, not person identity.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FrameInput {
    #[schemars(length(min = 1, max = 128), regex(pattern = "^[A-Za-z0-9_-]+$"))]
    frame_id: String,
}
impl FrameInput {
    pub(crate) fn new(frame_id: String) -> Self {
        Self { frame_id }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ListInput {
    #[schemars(length(min = 1, max = 128), regex(pattern = "^[A-Za-z0-9_-]+$"))]
    frame_id: String,
    #[schemars(length(min = 1, max = 128), regex(pattern = "^[A-Za-z0-9_-]+$"))]
    list_id: String,
}
impl ListInput {
    pub(crate) fn new(frame_id: String, list_id: String) -> Self {
        Self { frame_id, list_id }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EventsInput {
    #[schemars(length(min = 1, max = 128), regex(pattern = "^[A-Za-z0-9_-]+$"))]
    frame_id: String,
    #[schemars(regex(pattern = "^[0-9]{4}-[0-9]{2}-[0-9]{2}$"), extend("format" = "date"))]
    date_min: String,
    #[schemars(regex(pattern = "^[0-9]{4}-[0-9]{2}-[0-9]{2}$"), extend("format" = "date"))]
    date_max: String,
    #[schemars(length(min = 1, max = 128), regex(pattern = "^[A-Za-z0-9_+/-]+$"))]
    timezone: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_text"
    )]
    #[schemars(with = "String", extend("enum" = ["categories,calendar_account,event_notification_setting"]))]
    include: Option<String>,
}
impl EventsInput {
    pub(crate) fn new(
        frame_id: String,
        date_min: String,
        date_max: String,
        timezone: String,
        include: Option<String>,
    ) -> Self {
        Self {
            frame_id,
            date_min,
            date_max,
            timezone,
            include,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TasksInput {
    #[schemars(length(min = 1, max = 128), regex(pattern = "^[A-Za-z0-9_-]+$"))]
    frame_id: String,
    #[schemars(regex(pattern = "^[0-9]{4}-[0-9]{2}-[0-9]{2}$"), extend("format" = "date"))]
    after: String,
    #[schemars(regex(pattern = "^[0-9]{4}-[0-9]{2}-[0-9]{2}$"), extend("format" = "date"))]
    before: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_bool"
    )]
    #[schemars(with = "bool", extend("default" = false))]
    include_late: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_bool"
    )]
    #[schemars(with = "bool", extend("default" = false))]
    include_up_for_grabs: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_text"
    )]
    #[schemars(with = "String", extend("enum" = ["linked_to_profile"], "default" = "linked_to_profile"))]
    filter: Option<String>,
}
impl TasksInput {
    pub(crate) fn new(
        frame_id: String,
        after: String,
        before: String,
        include_late: Option<bool>,
        include_up_for_grabs: Option<bool>,
        filter: Option<String>,
    ) -> Self {
        Self {
            frame_id,
            after,
            before,
            include_late,
            include_up_for_grabs,
            filter,
        }
    }
}

fn present_text<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}
fn present_bool<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<bool>, D::Error> {
    bool::deserialize(deserializer).map(Some)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Read {
    Categories,
    Events,
    Lists,
    List,
    Items,
    Tasks,
}

#[cfg(test)]
pub(crate) const READS: [Read; 6] = [
    Read::Categories,
    Read::Events,
    Read::Lists,
    Read::List,
    Read::Items,
    Read::Tasks,
];
const MAX_RECORDS: usize = 64;
const EVENT_INCLUDE: &str = "categories,calendar_account,event_notification_setting";
const TASK_FILTER: &str = "linked_to_profile";

impl Read {
    #[cfg(test)]
    pub(crate) fn capability(self) -> &'static str {
        match self {
            Self::Categories => "skylight-private.categories.list",
            Self::Events => "skylight-private.calendar.events.list",
            Self::Lists => "skylight-private.lists.list",
            Self::List => "skylight-private.lists.read",
            Self::Items => "skylight-private.list.items.list",
            Self::Tasks => "skylight-private.tasks.list",
        }
    }

    #[cfg(test)]
    pub(crate) fn from_capability(id: &str) -> Option<Self> {
        READS.into_iter().find(|read| read.capability() == id)
    }

    pub(crate) fn fields(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Categories | Self::Lists => &[("--frame", "frameId")],
            Self::List | Self::Items => &[("--frame", "frameId"), ("--list", "listId")],
            Self::Events => &[
                ("--frame", "frameId"),
                ("--from", "dateMin"),
                ("--to", "dateMax"),
                ("--tz", "timezone"),
                ("--include", "include"),
            ],
            Self::Tasks => &[
                ("--frame", "frameId"),
                ("--after", "after"),
                ("--before", "before"),
                ("--include-late", "includeLate"),
                ("--include-up-for-grabs", "includeUpForGrabs"),
                ("--filter", "filter"),
            ],
        }
    }

    pub(crate) fn required(self, field: &str) -> bool {
        !matches!(
            field,
            "include" | "includeLate" | "includeUpForGrabs" | "filter"
        )
    }

    #[cfg(test)]
    pub(crate) fn manifest(self) -> dekopon_provider_sdk::ProviderCapability {
        let mut properties = serde_json::Map::new();
        for (_, field) in self.fields() {
            let schema = match *field {
                "frameId" | "listId" => {
                    json!({"type":"string", "minLength":1, "maxLength":128, "pattern":"^[A-Za-z0-9_-]+$"})
                }
                "dateMin" | "dateMax" | "after" | "before" => {
                    json!({"type":"string", "format":"date", "pattern":"^[0-9]{4}-[0-9]{2}-[0-9]{2}$"})
                }
                "include" => json!({"type":"string", "enum":[EVENT_INCLUDE]}),
                "filter" => json!({"type":"string", "enum":[TASK_FILTER], "default":TASK_FILTER}),
                "includeLate" | "includeUpForGrabs" => json!({"type":"boolean", "default":false}),
                _ => {
                    json!({"type":"string", "minLength":1, "maxLength":128, "pattern":"^[A-Za-z0-9_+/-]+$"})
                }
            };
            properties.insert((*field).to_owned(), schema);
        }
        dekopon_provider_sdk::ProviderCapability {
            id: self.capability().parse().expect("static capability"),
            description: match self {
                Self::Tasks => "Reads bounded source chores and assignment linkage; outstanding coverage unknown",
                Self::Categories => {
                    "Lists bounded categories and family-member linkage; not inferred person identities"
                }
                Self::Events => {
                    "Lists bounded calendar summaries and source times; completeness unknown"
                }
                Self::Lists => "Lists bounded list identifiers and labels; not Tasks",
                Self::List => "Reads a list identifier and label; not Tasks",
                Self::Items => {
                    "Lists bounded list item labels and source statuses; not dated assigned Tasks"
                }
            }
            .to_owned(),
            effect: EffectKind::ReadOnly,
            risk: RiskLevel::Medium,
            input_schema: json!({"type":"object", "properties":properties, "required":self.fields().iter().filter(|(_, key)| self.required(key)).map(|(_, key)| *key).collect::<Vec<_>>(), "additionalProperties":false}),
        }
    }

    pub(crate) fn uri(self, input: &Value) -> Result<String, ProviderError> {
        let object = input.as_object().ok_or_else(invalid_household_input)?;
        if object
            .keys()
            .any(|key| !self.fields().iter().any(|(_, field)| key == field))
            || self
                .fields()
                .iter()
                .any(|(_, field)| self.required(field) && !object.contains_key(*field))
        {
            return Err(invalid_household_input());
        }
        let field = |key: &str| {
            object
                .get(key)
                .and_then(Value::as_str)
                .ok_or_else(invalid_household_input)
        };
        let frame = field("frameId")?;
        safe_segment(frame)?;
        let base = format!("{FRAMES_URI}/{frame}");
        Ok(match self {
            Self::Categories => format!("{base}/categories"),
            Self::Lists => format!("{base}/lists"),
            Self::List | Self::Items => {
                let list = field("listId")?;
                safe_segment(list)?;
                format!(
                    "{base}/lists/{list}{}",
                    if self == Self::Items {
                        "/list_items"
                    } else {
                        ""
                    }
                )
            }
            Self::Tasks => {
                let after = field("after")?;
                let before = field("before")?;
                if !(0..=31).contains(&(ordinal(before)? - ordinal(after)?)) {
                    return Err(invalid_household_input());
                }
                let flag = |key| match object.get(key) {
                    None => Ok(false),
                    Some(Value::Bool(value)) => Ok(*value),
                    _ => Err(invalid_household_input()),
                };
                if object.contains_key("filter") && field("filter")? != TASK_FILTER {
                    return Err(invalid_household_input());
                }
                format!(
                    "{base}/chores?after={after}&before={before}&include_late={}&include_up_for_grabs={}&filter={TASK_FILTER}",
                    flag("includeLate")?,
                    flag("includeUpForGrabs")?
                )
            }
            Self::Events => {
                let from = field("dateMin")?;
                let to = field("dateMax")?;
                let days = ordinal(to)? - ordinal(from)?;
                if !(1..=31).contains(&days) {
                    return Err(invalid_household_input());
                }
                let timezone = field("timezone")?;
                if timezone.is_empty()
                    || timezone.len() > 128
                    || !timezone
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_+/-".contains(&b))
                    || timezone.split('/').any(str::is_empty)
                {
                    return Err(invalid_household_input());
                }
                let include = if object.contains_key("include") {
                    if field("include")? != EVENT_INCLUDE {
                        return Err(invalid_household_input());
                    }
                    format!("&include={}", encode_query(EVENT_INCLUDE))
                } else {
                    String::new()
                };
                format!(
                    "{base}/calendar_events?date_min={from}&date_max={to}&timezone={}{}",
                    encode_query(timezone),
                    include
                )
            }
        })
    }

    #[cfg(test)]
    pub(crate) fn invoke<F>(self, input: Value, send: F) -> Result<Value, ProviderError>
    where
        F: FnOnce(Request) -> Result<Response, HttpError>,
    {
        let uri = DEFAULT_BASE
            .join(&self.uri(&input)?)
            .map_err(|_| invalid_request())?;
        let body = send_once(method::GET, &uri, None, send)?.ok_or_else(invalid_response)?;
        self.project_body(&body)
    }

    pub(crate) fn project_body(self, body: &[u8]) -> Result<Value, ProviderError> {
        match self {
            Self::Categories => project(body, "categories", false, ProjectionKind::Category),
            Self::Events => project(body, "events", false, ProjectionKind::Event),
            Self::Lists => project(body, "lists", false, ProjectionKind::List),
            Self::List => project(body, "list", true, ProjectionKind::List),
            Self::Items => project(body, "items", false, ProjectionKind::Item),
            Self::Tasks => project(body, "tasks", false, ProjectionKind::Task),
        }
    }
}

// Calendar event writes. Selectors, bounds and body fields follow INVENTORY rows 7-9; the body is
// flat snake_case JSON with absent fields omitted, as pyskylight `client.py:168-185` sends it.
const EVENT_TIME_PATTERN: &str = "^[0-9]{4}-[0-9]{2}-[0-9]{2}(T[0-9]{2}:[0-9]{2}(:[0-9]{2}(\\.[0-9]{1,9})?)?(Z|[+-][0-9]{2}:[0-9]{2})?)?$";
const MAX_EVENT_TIME_BYTES: usize = 64;
const MAX_EVENT_LIST: usize = 16;

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EventCreateInput {
    #[schemars(length(min = 1, max = 128), regex(pattern = "^[A-Za-z0-9_-]+$"))]
    frame_id: String,
    #[schemars(length(min = 1, max = 256))]
    summary: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_text"
    )]
    #[schemars(with = "String", length(max = 64), regex(pattern = EVENT_TIME_PATTERN))]
    starts_at: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_text"
    )]
    #[schemars(with = "String", length(max = 64), regex(pattern = EVENT_TIME_PATTERN))]
    ends_at: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_bool"
    )]
    #[schemars(with = "bool")]
    all_day: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_text"
    )]
    #[schemars(
        with = "String",
        length(min = 1, max = 128),
        regex(pattern = "^[A-Za-z0-9_+/-]+$")
    )]
    timezone: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_texts"
    )]
    #[schemars(
        with = "Vec<String>",
        length(min = 1, max = 16),
        inner(length(min = 1, max = 128), regex(pattern = "^[A-Za-z0-9_-]+$")),
        extend("uniqueItems" = true)
    )]
    category_ids: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_texts"
    )]
    #[schemars(
        with = "Vec<String>",
        length(min = 1, max = 16),
        inner(length(min = 1, max = 256), regex(pattern = "^[ -~]+$"))
    )]
    rrule: Option<Vec<String>>,
}
impl EventCreateInput {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        frame_id: String,
        summary: String,
        starts_at: Option<String>,
        ends_at: Option<String>,
        all_day: Option<bool>,
        timezone: Option<String>,
        category_ids: Vec<String>,
        rrule: Vec<String>,
    ) -> Self {
        Self {
            frame_id,
            summary,
            starts_at,
            ends_at,
            all_day,
            timezone,
            category_ids: (!category_ids.is_empty()).then_some(category_ids),
            rrule: (!rrule.is_empty()).then_some(rrule),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[schemars(extend("minProperties" = 3))]
pub struct EventUpdateInput {
    #[schemars(length(min = 1, max = 128), regex(pattern = "^[A-Za-z0-9_-]+$"))]
    frame_id: String,
    #[schemars(length(min = 1, max = 128), regex(pattern = "^[A-Za-z0-9_-]+$"))]
    event_id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_text"
    )]
    #[schemars(with = "String", length(min = 1, max = 256))]
    summary: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_text"
    )]
    #[schemars(with = "String", length(max = 64), regex(pattern = EVENT_TIME_PATTERN))]
    starts_at: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_text"
    )]
    #[schemars(with = "String", length(max = 64), regex(pattern = EVENT_TIME_PATTERN))]
    ends_at: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_bool"
    )]
    #[schemars(with = "bool")]
    all_day: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_text"
    )]
    #[schemars(
        with = "String",
        length(min = 1, max = 128),
        regex(pattern = "^[A-Za-z0-9_+/-]+$")
    )]
    timezone: Option<String>,
}
impl EventUpdateInput {
    pub(crate) fn new(
        frame_id: String,
        event_id: String,
        summary: Option<String>,
        starts_at: Option<String>,
        ends_at: Option<String>,
        all_day: Option<bool>,
        timezone: Option<String>,
    ) -> Self {
        Self {
            frame_id,
            event_id,
            summary,
            starts_at,
            ends_at,
            all_day,
            timezone,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EventDeleteInput {
    #[schemars(length(min = 1, max = 128), regex(pattern = "^[A-Za-z0-9_-]+$"))]
    frame_id: String,
    #[schemars(length(min = 1, max = 128), regex(pattern = "^[A-Za-z0-9_-]+$"))]
    event_id: String,
}
impl EventDeleteInput {
    pub(crate) fn new(frame_id: String, event_id: String) -> Self {
        Self { frame_id, event_id }
    }
}

fn present_texts<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error> {
    Vec::<String>::deserialize(deserializer).map(Some)
}

/// The fixed write body: field order and names are the wire contract, never caller-chosen.
#[derive(Default, Serialize)]
struct EventBody<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    starts_at: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ends_at: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    all_day: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timezone: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    category_ids: Option<Vec<&'a str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rrule: Option<Vec<&'a str>>,
}

// List item writes: INVENTORY rows 23 and 38-40. Status is limited to the two values the item
// projection knows (`pending`, `completed`).
const MAX_ITEM_POSITION: u64 = 1_000_000;

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ItemCreateInput {
    #[schemars(length(min = 1, max = 128), regex(pattern = "^[A-Za-z0-9_-]+$"))]
    frame_id: String,
    #[schemars(length(min = 1, max = 128), regex(pattern = "^[A-Za-z0-9_-]+$"))]
    list_id: String,
    #[schemars(length(min = 1, max = 256))]
    label: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_text"
    )]
    #[schemars(with = "String", length(min = 1, max = 256))]
    section: Option<String>,
}
impl ItemCreateInput {
    pub(crate) fn new(
        frame_id: String,
        list_id: String,
        label: String,
        section: Option<String>,
    ) -> Self {
        Self {
            frame_id,
            list_id,
            label,
            section,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[schemars(extend("minProperties" = 4))]
pub struct ItemUpdateInput {
    #[schemars(length(min = 1, max = 128), regex(pattern = "^[A-Za-z0-9_-]+$"))]
    frame_id: String,
    #[schemars(length(min = 1, max = 128), regex(pattern = "^[A-Za-z0-9_-]+$"))]
    list_id: String,
    #[schemars(length(min = 1, max = 128), regex(pattern = "^[A-Za-z0-9_-]+$"))]
    item_id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_text"
    )]
    #[schemars(with = "String", length(min = 1, max = 256))]
    label: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_text"
    )]
    #[schemars(with = "String", extend("enum" = ["completed", "pending"]))]
    status: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_position"
    )]
    #[schemars(with = "u32", range(max = 1_000_000))]
    position: Option<u64>,
}
impl ItemUpdateInput {
    pub(crate) fn new(
        frame_id: String,
        list_id: String,
        item_id: String,
        label: Option<String>,
        status: Option<String>,
        position: Option<u64>,
    ) -> Self {
        Self {
            frame_id,
            list_id,
            item_id,
            label,
            status,
            position,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ItemCompleteInput {
    #[schemars(length(min = 1, max = 128), regex(pattern = "^[A-Za-z0-9_-]+$"))]
    frame_id: String,
    #[schemars(length(min = 1, max = 128), regex(pattern = "^[A-Za-z0-9_-]+$"))]
    list_id: String,
    #[schemars(length(min = 1, max = 128), regex(pattern = "^[A-Za-z0-9_-]+$"))]
    item_id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_text"
    )]
    #[schemars(with = "String", extend("enum" = ["completed", "pending"], "default" = "completed"))]
    status: Option<String>,
}
impl ItemCompleteInput {
    pub(crate) fn new(
        frame_id: String,
        list_id: String,
        item_id: String,
        status: Option<String>,
    ) -> Self {
        Self {
            frame_id,
            list_id,
            item_id,
            status,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ItemDeleteInput {
    #[schemars(length(min = 1, max = 128), regex(pattern = "^[A-Za-z0-9_-]+$"))]
    frame_id: String,
    #[schemars(length(min = 1, max = 128), regex(pattern = "^[A-Za-z0-9_-]+$"))]
    list_id: String,
    #[schemars(length(min = 1, max = 128), regex(pattern = "^[A-Za-z0-9_-]+$"))]
    item_id: String,
}
impl ItemDeleteInput {
    pub(crate) fn new(frame_id: String, list_id: String, item_id: String) -> Self {
        Self {
            frame_id,
            list_id,
            item_id,
        }
    }
}

fn present_position<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u64>, D::Error> {
    u64::deserialize(deserializer).map(Some)
}

/// The fixed list item body, in pyskylight's field order.
#[derive(Default, Serialize)]
struct ItemBody<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    section: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<u64>,
}

// Variants name resource then verb; later batches add other resources' writes here.
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mutation {
    EventCreate,
    EventUpdate,
    EventDelete,
    ItemCreate,
    ItemUpdate,
    ItemComplete,
    ItemDelete,
}

#[cfg(test)]
pub(crate) const WRITES: [Mutation; 7] = [
    Mutation::EventCreate,
    Mutation::EventUpdate,
    Mutation::EventDelete,
    Mutation::ItemCreate,
    Mutation::ItemUpdate,
    Mutation::ItemComplete,
    Mutation::ItemDelete,
];

impl Mutation {
    #[cfg(test)]
    pub(crate) fn capability(self) -> &'static str {
        match self {
            Self::EventCreate => "skylight-private.calendar.events.create",
            Self::EventUpdate => "skylight-private.calendar.events.update",
            Self::EventDelete => "skylight-private.calendar.events.delete",
            Self::ItemCreate => "skylight-private.list.items.create",
            Self::ItemUpdate => "skylight-private.list.items.update",
            Self::ItemComplete => "skylight-private.list.items.complete",
            Self::ItemDelete => "skylight-private.list.items.delete",
        }
    }

    #[cfg(test)]
    pub(crate) fn from_capability(id: &str) -> Option<Self> {
        WRITES.into_iter().find(|write| write.capability() == id)
    }

    #[cfg(test)]
    pub(crate) fn method(self) -> &'static str {
        match self {
            Self::EventCreate | Self::ItemCreate => method::POST,
            Self::EventUpdate | Self::ItemUpdate | Self::ItemComplete => method::PUT,
            Self::EventDelete | Self::ItemDelete => method::DELETE,
        }
    }

    /// Every accepted input field; the first group is required.
    fn fields(self) -> (&'static [&'static str], &'static [&'static str]) {
        match self {
            Self::EventCreate => (
                &["frameId", "summary"],
                &[
                    "startsAt",
                    "endsAt",
                    "allDay",
                    "timezone",
                    "categoryIds",
                    "rrule",
                ],
            ),
            Self::EventUpdate => (
                &["frameId", "eventId"],
                &["summary", "startsAt", "endsAt", "allDay", "timezone"],
            ),
            Self::EventDelete => (&["frameId", "eventId"], &[]),
            Self::ItemCreate => (&["frameId", "listId", "label"], &["section"]),
            Self::ItemUpdate => (
                &["frameId", "listId", "itemId"],
                &["label", "status", "position"],
            ),
            Self::ItemComplete => (&["frameId", "listId", "itemId"], &["status"]),
            Self::ItemDelete => (&["frameId", "listId", "itemId"], &[]),
        }
    }

    /// Validates the whole input and returns the fixed route and body; both public halves call it,
    /// so neither can run on an input the other would refuse.
    fn request(self, input: &Value) -> Result<(String, Option<Vec<u8>>), ProviderError> {
        let object = input.as_object().ok_or_else(invalid_household_input)?;
        let (required, optional) = self.fields();
        if object
            .keys()
            .any(|key| !required.contains(&key.as_str()) && !optional.contains(&key.as_str()))
            || required.iter().any(|key| !object.contains_key(*key))
            || (matches!(self, Self::EventUpdate | Self::ItemUpdate)
                && !optional.iter().any(|key| object.contains_key(*key)))
        {
            return Err(invalid_household_input());
        }
        let text = |key: &str| match object.get(key) {
            None => Ok(None),
            Some(Value::String(value)) => Ok(Some(value.as_str())),
            Some(_) => Err(invalid_household_input()),
        };
        let selector = |key: &str| {
            let value = text(key)?.ok_or_else(invalid_household_input)?;
            safe_segment(value)?;
            Ok::<_, ProviderError>(value)
        };
        let frame = selector("frameId")?;
        if let Self::ItemCreate | Self::ItemUpdate | Self::ItemComplete | Self::ItemDelete = self {
            let base = format!(
                "{FRAMES_URI}/{frame}/lists/{}/list_items",
                selector("listId")?
            );
            if self == Self::ItemCreate {
                let body = ItemBody {
                    label: Some(
                        text("label")?
                            .filter(|label| valid_summary(label))
                            .ok_or_else(invalid_household_input)?,
                    ),
                    section: match text("section")? {
                        Some(section) if !valid_summary(section) => {
                            return Err(invalid_household_input());
                        }
                        section => section,
                    },
                    ..ItemBody::default()
                };
                return Ok((base, Some(encode(&body)?)));
            }
            let path = format!("{base}/{}", selector("itemId")?);
            let status = match text("status")? {
                None if self == Self::ItemComplete => Some(ITEM_COMPLETED),
                Some(status @ (ITEM_COMPLETED | ITEM_PENDING)) => Some(status),
                None => None,
                Some(_) => return Err(invalid_household_input()),
            };
            let body = match self {
                Self::ItemDelete => return Ok((path, None)),
                Self::ItemComplete => ItemBody {
                    status,
                    ..ItemBody::default()
                },
                _ => ItemBody {
                    label: match text("label")? {
                        Some(label) if !valid_summary(label) => {
                            return Err(invalid_household_input());
                        }
                        label => label,
                    },
                    status,
                    position: match object.get("position") {
                        None => None,
                        Some(value) => Some(
                            value
                                .as_u64()
                                .filter(|position| *position <= MAX_ITEM_POSITION)
                                .ok_or_else(invalid_household_input)?,
                        ),
                    },
                    ..ItemBody::default()
                },
            };
            return Ok((path, Some(encode(&body)?)));
        }
        let base = format!("{FRAMES_URI}/{frame}/calendar_events");
        if self == Self::EventDelete {
            let event = selector("eventId")?;
            return Ok((format!("{base}/{event}"), None));
        }
        let texts = |key: &str, valid: fn(&str) -> bool, unique: bool| match object.get(key) {
            None => Ok(None),
            Some(Value::Array(values)) if (1..=MAX_EVENT_LIST).contains(&values.len()) => {
                let mut seen = HashSet::new();
                values
                    .iter()
                    .map(|value| match value.as_str() {
                        Some(value) if valid(value) && (!unique || seen.insert(value)) => Ok(value),
                        _ => Err(invalid_household_input()),
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map(Some)
            }
            Some(_) => Err(invalid_household_input()),
        };
        let checked = |key: &str, valid: fn(&str) -> bool| match text(key)? {
            Some(value) if !valid(value) => Err(invalid_household_input()),
            value => Ok(value),
        };
        let body = EventBody {
            summary: checked("summary", valid_summary)?,
            starts_at: checked("startsAt", valid_event_time)?,
            ends_at: checked("endsAt", valid_event_time)?,
            all_day: match object.get("allDay") {
                None => None,
                Some(Value::Bool(value)) => Some(*value),
                Some(_) => return Err(invalid_household_input()),
            },
            timezone: checked("timezone", valid_timezone)?,
            category_ids: texts("categoryIds", |id| safe_segment(id).is_ok(), true)?,
            rrule: texts("rrule", valid_rule, false)?,
        };
        let path = if self == Self::EventUpdate {
            format!("{base}/{}", selector("eventId")?)
        } else {
            base
        };
        Ok((path, Some(encode(&body)?)))
    }

    pub(crate) fn uri(self, input: &Value) -> Result<String, ProviderError> {
        self.request(input).map(|(path, _)| path)
    }

    pub(crate) fn body(self, input: &Value) -> Result<Option<Vec<u8>>, ProviderError> {
        self.request(input).map(|(_, body)| body)
    }

    #[cfg(test)]
    pub(crate) fn invoke<F>(self, input: Value, send: F) -> Result<Value, ProviderError>
    where
        F: FnOnce(Request) -> Result<Response, HttpError>,
    {
        let (path, body) = self.request(&input)?;
        let uri = DEFAULT_BASE.join(&path).map_err(|_| invalid_request())?;
        match send_once(self.method(), &uri, body, send)? {
            Some(body) => self.project_body(&body),
            None => Ok(no_content_output()),
        }
    }

    /// A 200 or 201 projects the written record, DELETE included. The write has applied by then,
    /// so a record that cannot be projected still succeeds, marked `recordProjected:false`,
    /// rather than failing and inviting a retry that would repeat the write.
    pub(crate) fn project_body(self, body: &[u8]) -> Result<Value, ProviderError> {
        let projected = match self {
            Self::EventCreate | Self::EventUpdate | Self::EventDelete => {
                project(body, "event", true, ProjectionKind::Event)
            }
            Self::ItemCreate | Self::ItemUpdate | Self::ItemComplete | Self::ItemDelete => {
                project(body, "item", true, ProjectionKind::Item)
            }
        };
        Ok(projected.unwrap_or_else(|_| json!({"ok": true, "recordProjected": false})))
    }
}

const ITEM_COMPLETED: &str = "completed";
const ITEM_PENDING: &str = "pending";

fn encode<T: Serialize>(body: &T) -> Result<Vec<u8>, ProviderError> {
    serde_json::to_vec(body).map_err(|_| invalid_household_input())
}

fn valid_summary(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_NAME_BYTES
        && !value.chars().any(char::is_control)
}

fn valid_timezone(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_+/-".contains(&b))
        && !value.split('/').any(str::is_empty)
}

fn valid_rule(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_NAME_BYTES
        && value.bytes().all(|b| (b' '..=b'~').contains(&b))
}

/// `YYYY-MM-DD`, or that date with `THH:MM[:SS[.fraction]]` and an optional `Z` or `±HH:MM`.
fn valid_event_time(value: &str) -> bool {
    fn two(bytes: &[u8], max: u8) -> bool {
        bytes.len() == 2
            && bytes.iter().all(u8::is_ascii_digit)
            && (bytes[0] - b'0') * 10 + (bytes[1] - b'0') <= max
    }
    if !value.is_ascii()
        || value.len() > MAX_EVENT_TIME_BYTES
        || value.len() < 10
        || ordinal(&value[..10]).is_err()
    {
        return false;
    }
    let rest = &value.as_bytes()[10..];
    if rest.is_empty() {
        return true;
    }
    if rest.len() < 6
        || rest[0] != b'T'
        || !two(&rest[1..3], 23)
        || rest[3] != b':'
        || !two(&rest[4..6], 59)
    {
        return false;
    }
    let mut rest = &rest[6..];
    if rest.first() == Some(&b':') {
        if rest.len() < 3 || !two(&rest[1..3], 59) {
            return false;
        }
        rest = &rest[3..];
        if rest.first() == Some(&b'.') {
            let digits = rest[1..].iter().take_while(|b| b.is_ascii_digit()).count();
            if !(1..=9).contains(&digits) {
                return false;
            }
            rest = &rest[1 + digits..];
        }
    }
    match rest {
        [] | [b'Z'] => true,
        [sign, h1, h2, b':', m1, m2] if b"+-".contains(sign) => {
            two(&[*h1, *h2], 23) && two(&[*m1, *m2], 59)
        }
        _ => false,
    }
}

pub(crate) fn invalid_household_input() -> ProviderError {
    ProviderError::new(
        "invalid-input",
        "the input does not match the capability's input schema",
    )
}

fn safe_segment(value: &str) -> Result<(), ProviderError> {
    if value.is_empty()
        || value.len() > MAX_ID_BYTES
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err(invalid_household_input());
    }
    Ok(())
}

fn encode_query(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(encoded, "%{byte:02X}").expect("write to string");
        }
    }
    encoded
}

/// Gregorian date validation without a clock, timezone database or local-time assumptions.
fn ordinal(date: &str) -> Result<i32, ProviderError> {
    let b = date.as_bytes();
    if b.len() != 10
        || b[4] != b'-'
        || b[7] != b'-'
        || b.iter()
            .enumerate()
            .any(|(i, b)| i != 4 && i != 7 && !b.is_ascii_digit())
    {
        return Err(invalid_household_input());
    }
    let year: i32 = date[..4].parse().map_err(|_| invalid_household_input())?;
    let month: usize = date[5..7].parse().map_err(|_| invalid_household_input())?;
    let day: i32 = date[8..].parse().map_err(|_| invalid_household_input())?;
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let months = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if year == 0 || !(1..=12).contains(&month) || day < 1 || day > months[month - 1] {
        return Err(invalid_household_input());
    }
    let y = year - 1;
    Ok(365 * y + y / 4 - y / 100 + y / 400 + months[..month - 1].iter().sum::<i32>() + day)
}

/// Unlike Option fields, this wrapper lets serde detect duplicates even after a JSON null.
#[derive(Debug, Deserialize)]
#[serde(transparent)]
struct Nullable<T>(Option<T>);
impl<T> Default for Nullable<T> {
    fn default() -> Self {
        Self(None)
    }
}

fn text(output: &mut Value, key: &str, value: Nullable<String>) -> bool {
    let (value, truncated) = match value.0 {
        Some(value) => {
            let (s, t) = truncate_name(&value);
            (Value::String(s), t)
        }
        None => (Value::Null, false),
    };
    output[key] = value;
    truncated
}

// Rules are opaque source strings. Validate discarded tails, never expand recurrence.
#[derive(Default)]
struct Rules {
    values: Vec<String>,
    total: usize,
}
impl<'de> Deserialize<'de> for Rules {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Rules;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("string array")
            }
            fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<Rules, S::Error> {
                let mut result = Rules::default();
                while let Some(value) = seq.next_element::<String>()? {
                    if value.len() > MAX_NAME_BYTES {
                        return Err(serde::de::Error::custom("long rule"));
                    }
                    result.total += 1;
                    if result.values.len() < 16 {
                        result.values.push(value);
                    }
                }
                Ok(result)
            }
        }
        d.deserialize_seq(V)
    }
}
#[derive(Default, Deserialize)]
struct Attributes {
    #[serde(default)]
    label: Nullable<String>,
    #[serde(default)]
    color: Nullable<String>,
    #[serde(default)]
    kind: Nullable<String>,
    #[serde(default)]
    linked_to_profile: Nullable<bool>,
    #[serde(default)]
    selected_for_chore_chart: Nullable<bool>,
    #[serde(default)]
    hide_on_device: Nullable<bool>,
    #[serde(default)]
    draft: Nullable<bool>,
    #[serde(default)]
    default_grocery_list: Nullable<bool>,
    #[serde(default)]
    summary: Nullable<String>,
    #[serde(default)]
    description: Nullable<String>,
    #[serde(default)]
    location: Nullable<String>,
    #[serde(default)]
    status: Nullable<String>,
    #[serde(default)]
    starts_at: Nullable<String>,
    #[serde(default)]
    ends_at: Nullable<String>,
    #[serde(default)]
    timezone: Nullable<String>,
    #[serde(default)]
    all_day: Nullable<bool>,
    #[serde(default)]
    recurring: Nullable<bool>,
    #[serde(default)]
    recurring_config: Nullable<bool>,
    #[serde(default)]
    rrule: Nullable<Rules>,
    #[serde(default)]
    section: Nullable<String>,
    #[serde(default)]
    created_at: Nullable<String>,
    #[serde(default)]
    position: Nullable<serde_json::Number>,
    #[serde(default)]
    origin: Nullable<String>,
    #[serde(default)]
    group: Nullable<String>,
    #[serde(default)]
    series: Nullable<String>,
    #[serde(default)]
    start: Nullable<String>,
    #[serde(default)]
    completed_at: Nullable<String>,
    #[serde(default)]
    routine: Nullable<bool>,
    #[serde(default)]
    up_for_grabs: Nullable<bool>,
    #[serde(default)]
    recurrence_set: Nullable<Rules>,
    #[serde(default)]
    completed_on: Nullable<serde::de::IgnoredAny>,
    #[serde(default)]
    recurring_until: Nullable<serde::de::IgnoredAny>,
    #[serde(default)]
    start_time: Nullable<serde::de::IgnoredAny>,
    #[serde(default)]
    name: Nullable<String>,
}
#[derive(Clone, Copy)]
enum ProjectionKind {
    Category,
    List,
    Event,
    Item,
    Task,
    Included,
}
impl Attributes {
    fn text_truncated(&self) -> bool {
        [
            &self.label,
            &self.color,
            &self.kind,
            &self.summary,
            &self.description,
            &self.location,
            &self.status,
            &self.section,
            &self.origin,
            &self.name,
        ]
        .iter()
        .any(|v| v.0.as_ref().is_some_and(|s| s.len() > MAX_NAME_BYTES))
    }
    fn validate(&self) -> Result<bool, ProviderError> {
        for value in [
            &self.starts_at,
            &self.ends_at,
            &self.timezone,
            &self.created_at,
            &self.group,
            &self.series,
            &self.start,
            &self.completed_at,
        ] {
            if value.0.as_ref().is_some_and(|s| s.len() > MAX_NAME_BYTES) {
                return Err(invalid_response());
            }
        }
        Ok(self.text_truncated()
            || [&self.rrule, &self.recurrence_set]
                .iter()
                .any(|r| r.0.as_ref().is_some_and(|r| r.total > r.values.len())))
    }
    fn fields(self, kind: ProjectionKind) -> Result<Value, ProviderError> {
        match kind {
            ProjectionKind::Category => {
                let mut truncated = self.text_truncated();
                let mut output = json!({});
                truncated |= text(&mut output, "label", self.label);
                truncated |= text(&mut output, "color", self.color);
                output["linkedToProfile"] = json!(self.linked_to_profile.0);
                output["selectedForChoreChart"] = json!(self.selected_for_chore_chart.0);
                output["textTruncated"] = json!(truncated);
                Ok(output)
            }
            ProjectionKind::List => {
                let mut truncated = self.text_truncated();
                let mut output = json!({});
                truncated |= text(&mut output, "label", self.label);
                truncated |= text(&mut output, "color", self.color);
                truncated |= text(&mut output, "kind", self.kind);
                output["hideOnDevice"] = json!(self.hide_on_device.0);
                output["draft"] = json!(self.draft.0);
                output["defaultGroceryList"] = json!(self.default_grocery_list.0);
                output["textTruncated"] = json!(truncated);
                Ok(output)
            }
            ProjectionKind::Event => {
                let mut truncated = self.text_truncated();
                let mut output = json!({});
                truncated |= text(&mut output, "summary", self.summary);
                truncated |= text(&mut output, "description", self.description);
                truncated |= text(&mut output, "location", self.location);
                truncated |= text(&mut output, "status", self.status);
                output["startsAt"] = json!(self.starts_at.0);
                output["endsAt"] = json!(self.ends_at.0);
                output["timezone"] = json!(self.timezone.0);
                output["allDay"] = json!(self.all_day.0);
                output["recurring"] = json!(self.recurring.0);
                output["recurringConfig"] = json!(self.recurring_config.0);
                output["rruleTruncated"] = json!(
                    self.rrule
                        .0
                        .as_ref()
                        .is_some_and(|r| r.total > r.values.len())
                );
                output["rrule"] = json!(self.rrule.0.map(|r| r.values));
                output["textTruncated"] = json!(truncated);
                Ok(output)
            }
            ProjectionKind::Item => {
                let mut truncated = self.text_truncated();
                let mut output = json!({});
                truncated |= text(&mut output, "label", self.label);
                output["status"] = match self.status.0.as_deref() {
                    Some("pending") => json!("pending"),
                    Some("completed") => json!("completed"),
                    _ => Value::Null,
                };
                truncated |= text(&mut output, "sourceStatus", self.status);
                truncated |= text(&mut output, "section", self.section);
                output["createdAt"] = json!(self.created_at.0);
                output["draft"] = json!(self.draft.0);
                output["position"] = json!(self.position.0);
                output["textTruncated"] = json!(truncated);
                Ok(output)
            }
            ProjectionKind::Task => {
                let mut truncated = self.text_truncated();
                let mut output = json!({});
                truncated |= text(&mut output, "summary", self.summary);
                truncated |= text(&mut output, "description", self.description);
                truncated |= text(&mut output, "status", self.status);
                truncated |= text(&mut output, "origin", self.origin);
                output["group"] = json!(self.group.0);
                output["series"] = json!(self.series.0);
                output["start"] = json!(self.start.0);
                output["completedAt"] = json!(self.completed_at.0);
                output["recurring"] = json!(self.recurring.0);
                output["routine"] = json!(self.routine.0);
                output["upForGrabs"] = json!(self.up_for_grabs.0);
                output["position"] = json!(self.position.0);
                output["recurrenceSetTruncated"] = json!(
                    self.recurrence_set
                        .0
                        .as_ref()
                        .is_some_and(|r| r.total > r.values.len())
                );
                output["recurrenceSet"] = json!(self.recurrence_set.0.map(|r| r.values));
                output["completedOnState"] = json!(if self.completed_on.0.is_some() {
                    "unverified-non-null"
                } else {
                    "null-or-missing"
                });
                output["recurringUntilState"] = json!(if self.recurring_until.0.is_some() {
                    "unverified-non-null"
                } else {
                    "null-or-missing"
                });
                output["startTimeState"] = json!(if self.start_time.0.is_some() {
                    "unverified-non-null"
                } else {
                    "null-or-missing"
                });
                output["textTruncated"] = json!(truncated);
                Ok(output)
            }
            ProjectionKind::Included => {
                let mut truncated = self.text_truncated();
                let mut output = json!({});
                truncated |= text(&mut output, "label", self.label);
                truncated |= text(&mut output, "status", self.status);
                truncated |= text(&mut output, "section", self.section);
                output["createdAt"] = json!(self.created_at.0);
                output["draft"] = json!(self.draft.0);
                output["position"] = json!(self.position.0);
                truncated |= text(&mut output, "color", self.color);
                truncated |= text(&mut output, "kind", self.kind);
                truncated |= text(&mut output, "name", self.name);
                output["linkedToProfile"] = json!(self.linked_to_profile.0);
                output["attributesCoverage"] =
                    json!("known-fields-only; unverified-resource-attributes-unknown");
                output["textTruncated"] = json!(truncated);
                Ok(output)
            }
        }
    }
}

// A linkage is retained by (type,id); it is never promoted to a person identity.
#[derive(Deserialize)]
struct Link {
    id: String,
    #[serde(rename = "type")]
    kind: String,
}
impl Link {
    fn validate(&self) -> Result<(), ProviderError> {
        validate_id(&self.id)?;
        validate_id(&self.kind)
    }
    fn fields(self) -> Value {
        json!({"type":self.kind,"id":self.id,"included":false})
    }
}
#[derive(Default)]
struct Links {
    selected: BTreeMap<(String, String), Link>,
    total: usize,
}
impl<'de> Deserialize<'de> for Links {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Links;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("linkage array")
            }
            fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<Links, S::Error> {
                let mut result = Links::default();
                let mut seen = HashSet::new();
                while let Some(JsonObject(link)) = seq.next_element::<JsonObject<Link>>()? {
                    link.validate().map_err(serde::de::Error::custom)?;
                    let key = (link.kind.clone(), link.id.clone());
                    if !seen.insert(key.clone()) {
                        return Err(serde::de::Error::custom("duplicate linkage"));
                    }
                    result.total += 1;
                    if result.selected.len() < 16
                        || result
                            .selected
                            .last_key_value()
                            .is_some_and(|(k, _)| key < *k)
                    {
                        result.selected.insert(key, link);
                        if result.selected.len() > 16 {
                            result.selected.pop_last();
                        }
                    }
                }
                Ok(result)
            }
        }
        d.deserialize_seq(V)
    }
}
#[derive(Deserialize)]
struct One {
    data: Nullable<JsonObject<Link>>,
}
#[derive(Deserialize)]
struct Many {
    data: Nullable<Links>,
}
#[derive(Default, Deserialize)]
struct Relationships {
    #[serde(default)]
    category: Nullable<JsonObject<One>>,
    #[serde(default)]
    completed_category: Nullable<JsonObject<One>>,
    #[serde(default)]
    family_member: Nullable<JsonObject<One>>,
    #[serde(default)]
    calendar_account: Nullable<JsonObject<One>>,
    #[serde(default)]
    event_notification_setting: Nullable<JsonObject<One>>,
    #[serde(default)]
    list: Nullable<JsonObject<One>>,
    #[serde(default)]
    avatar: Nullable<JsonObject<One>>,
    #[serde(default)]
    habit_tracker: Nullable<JsonObject<One>>,
    #[serde(default)]
    categories: Nullable<JsonObject<Many>>,
    #[serde(default)]
    list_items: Nullable<JsonObject<Many>>,
}
impl Relationships {
    fn validate(&self) -> Result<bool, ProviderError> {
        for relationship in [
            &self.category,
            &self.completed_category,
            &self.family_member,
            &self.calendar_account,
            &self.event_notification_setting,
            &self.list,
            &self.avatar,
            &self.habit_tracker,
        ] {
            if let Some(link) = relationship.0.as_ref().and_then(|r| r.0.data.0.as_ref()) {
                link.0.validate()?;
            }
        }
        Ok([&self.categories, &self.list_items].iter().any(|r| {
            r.0.as_ref()
                .and_then(|r| r.0.data.0.as_ref())
                .is_some_and(|links| links.total > links.selected.len())
        }))
    }
    fn fields(self) -> Value {
        let mut output = json!({});
        for (key, relationship) in [
            ("category", self.category),
            ("completed_category", self.completed_category),
            ("family_member", self.family_member),
            ("calendar_account", self.calendar_account),
            (
                "event_notification_setting",
                self.event_notification_setting,
            ),
            ("list", self.list),
            ("avatar", self.avatar),
            ("habit_tracker", self.habit_tracker),
        ] {
            if let Some(r) = relationship.0 {
                output[key] = json!({"data":r.0.data.0.map(|l| l.0.fields())});
            }
        }
        for (key, relationship) in [
            ("categories", self.categories),
            ("list_items", self.list_items),
        ] {
            if let Some(r) = relationship.0 {
                output[key] = match r.0.data.0 {
                    None => json!({"data":null,"truncated":false}),
                    Some(links) => {
                        json!({"truncated":links.total > links.selected.len(), "data":links.selected.into_values().map(Link::fields).collect::<Vec<_>>()})
                    }
                };
            }
        }
        output
    }
}

// Absence avoids constructing the large typed attribute set for minimal records;
// explicit null still fails the original object-only contract.
#[derive(Default)]
struct MissingAttributes(Option<Box<JsonObject<Attributes>>>);
impl<'de> Deserialize<'de> for MissingAttributes {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        JsonObject::<Attributes>::deserialize(d).map(|a| Self(Some(Box::new(a))))
    }
}
#[derive(Deserialize)]
struct Resource {
    id: String,
    #[serde(default, rename = "type")]
    kind: Nullable<String>,
    #[serde(default)]
    attributes: MissingAttributes,
    #[serde(default)]
    relationships: Nullable<Box<JsonObject<Relationships>>>,
}
impl Resource {
    fn validate(&self) -> Result<bool, ProviderError> {
        validate_id(&self.id)?;
        if let Some(kind) = &self.kind.0 {
            validate_id(kind)?;
        }
        Ok(self
            .attributes
            .0
            .as_ref()
            .map_or(Ok(false), |a| a.0.validate())?
            | self
                .relationships
                .0
                .as_ref()
                .map_or(Ok(false), |r| r.0.validate())?)
    }
    fn fields(self, kind: ProjectionKind) -> Result<Value, ProviderError> {
        let truncated = self.validate()?;
        let mut record = self
            .attributes
            .0
            .map(|a| a.0)
            .unwrap_or_default()
            .fields(kind)?;
        if matches!(kind, ProjectionKind::Included) {
            let fields: &[&str] = match self.kind.0.as_deref() {
                Some("category") => &["label", "color", "linkedToProfile"],
                Some("list_item") => &[
                    "label",
                    "status",
                    "section",
                    "createdAt",
                    "draft",
                    "position",
                ],
                Some("avatar") => &["name", "kind"],
                // Family-member/calendar-account/notification and future resource attributes
                // are unverified. Retain their identity/linkage, never invent a typed name.
                _ => &[],
            };
            record
                .as_object_mut()
                .expect("typed projection object")
                .retain(|key, _| {
                    fields.contains(&key.as_str())
                        || matches!(key.as_str(), "attributesCoverage" | "textTruncated")
                });
        }
        record["id"] = json!(self.id);
        if let Some(kind) = self.kind.0 {
            record["type"] = json!(kind);
        }
        if let Some(r) = self.relationships.0 {
            record["relationships"] = r.0.fields();
        }
        record["truncated"] = json!(truncated);
        Ok(record)
    }
}

#[derive(Default, Deserialize)]
struct Metadata {
    #[serde(default)]
    sections: Nullable<Sections>,
}
// Nonempty section element schemas are unverified: consume, count, never forward JSON.
#[derive(Default)]
struct Sections(usize);
impl<'de> Deserialize<'de> for Sections {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Sections;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("sections array")
            }
            fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<Sections, S::Error> {
                let mut n = 0;
                while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {
                    n += 1;
                }
                Ok(Sections(n))
            }
        }
        d.deserialize_seq(V)
    }
}
#[derive(Deserialize)]
struct Envelope<T> {
    data: T,
    #[serde(default)]
    included: Nullable<Resources<true>>,
    #[serde(default)]
    meta: Nullable<JsonObject<Metadata>>,
}

struct Resources<const INCLUDED: bool = false> {
    selected: BTreeMap<String, Resource>,
    total: usize,
    text_truncated: bool,
}
impl<'de, const INCLUDED: bool> Deserialize<'de> for Resources<INCLUDED> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ResourcesVisitor<const INCLUDED: bool>;
        impl<'de, const INCLUDED: bool> Visitor<'de> for ResourcesVisitor<INCLUDED> {
            type Value = Resources<INCLUDED>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a resource array")
            }
            fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<Self::Value, S::Error> {
                let mut result = Resources {
                    selected: BTreeMap::new(),
                    total: 0,
                    text_truncated: false,
                };
                let mut seen = HashSet::new();
                while let Some(JsonObject(resource)) = seq.next_element::<JsonObject<Resource>>()? {
                    result.text_truncated |=
                        resource.validate().map_err(serde::de::Error::custom)?;
                    let key = if INCLUDED {
                        let kind = resource
                            .kind
                            .0
                            .as_ref()
                            .ok_or_else(|| serde::de::Error::custom("missing included type"))?;
                        // Length-prefix avoids ambiguous keys even for opaque source identifiers.
                        format!("{}:{kind}{}", kind.len(), resource.id)
                    } else {
                        resource.id.clone()
                    };
                    if !seen.insert(key.clone()) {
                        return Err(serde::de::Error::custom("duplicate identifier"));
                    }
                    result.total += 1;
                    // Retain typed resources, project only final selection (including descending input).
                    if result.selected.len() < MAX_RECORDS
                        || result
                            .selected
                            .last_key_value()
                            .is_some_and(|(id, _)| key < *id)
                    {
                        result.selected.insert(key, resource);
                        if result.selected.len() > MAX_RECORDS {
                            result.selected.pop_last();
                        }
                    }
                }
                Ok(result)
            }
        }
        deserializer.deserialize_seq(ResourcesVisitor::<INCLUDED>)
    }
}
fn project(
    body: &[u8],
    key: &str,
    single: bool,
    kind: ProjectionKind,
) -> Result<Value, ProviderError> {
    let (mut records, total, text_truncated, included, meta) = if single {
        let e: JsonObject<Envelope<JsonObject<Resource>>> =
            serde_json::from_slice(body).map_err(|_| invalid_response())?;
        let r = e.0.data.0;
        let truncated = r.validate()?;
        (vec![r.fields(kind)?], 1, truncated, e.0.included, e.0.meta)
    } else {
        let e: JsonObject<Envelope<Resources>> =
            serde_json::from_slice(body).map_err(|_| invalid_response())?;
        (
            e.0.data
                .selected
                .into_values()
                .map(|r| r.fields(kind))
                .collect::<Result<Vec<_>, _>>()?,
            e.0.data.total,
            e.0.data.text_truncated,
            e.0.included,
            e.0.meta,
        )
    };
    let included_known = included.0.is_some();
    let (mut included, included_total, included_truncated) = match included.0 {
        Some(r) => (
            r.selected
                .into_values()
                .map(|r| r.fields(ProjectionKind::Included))
                .collect::<Result<Vec<_>, _>>()?,
            r.total,
            r.text_truncated,
        ),
        None => (Vec::new(), 0, false),
    };
    let sections = meta.0.and_then(|m| m.0.sections.0).map(|s| s.0);
    let mut output = json!({key:if single { Value::Null } else { json!([]) },
        "included":[], "includedState":if included_known { "present" } else { "null-or-missing" },
        "sectionsCount":sections, "sectionsSchema":"unknown", "projection":"typed-subset",
        "linkageResolution":"included flag matches retained type/id; false means unresolved, not unassigned",
        "relationshipsCoverage":"known-linkage-only; absent-or-null-relationship-unknown",
        "truncated":false, "coverage":"bounded-response", "upstreamCompleteness":"unknown"});
    // Serialize each final candidate once to account for escaping. Repeatedly cloning and
    // serializing the entire envelope would make rich included/linkage responses quadratic.
    let lengths = |values: &[Value]| {
        values
            .iter()
            .map(|v| {
                serde_json::to_vec(v)
                    .map(|b| b.len())
                    .map_err(|_| invalid_response())
            })
            .collect::<Result<Vec<_>, _>>()
    };
    let mut record_lengths = lengths(&records)?;
    let mut included_lengths = lengths(&included)?;
    let array_bytes =
        |lengths: &[usize]| lengths.iter().sum::<usize>() + lengths.len().saturating_sub(1);
    let base_bytes = serde_json::to_vec(&output)
        .map_err(|_| invalid_response())?
        .len();
    while base_bytes + array_bytes(&record_lengths) + array_bytes(&included_lengths)
        > MAX_PROJECTED_OUTPUT_BYTES
    {
        // Null's four bytes in single-record envelopes conservatively reserve extra space.
        if included.pop().is_some() {
            included_lengths.pop();
        } else {
            records.pop().ok_or_else(invalid_response)?;
            record_lengths.pop();
        }
    }
    output["truncated"] = json!(
        text_truncated
            || included_truncated
            || records.len() < total
            || included.len() < included_total
            || sections.is_some_and(|n| n > 0)
    );
    let identities: HashSet<(String, String)> = included
        .iter()
        .filter_map(|r| Some((r["type"].as_str()?.to_owned(), r["id"].as_str()?.to_owned())))
        .collect();
    for record in records.iter_mut().chain(included.iter_mut()) {
        resolve_linkage(record, &identities);
    }
    output[key] = if single {
        records.pop().unwrap_or(Value::Null)
    } else {
        Value::Array(records)
    };
    output["included"] = Value::Array(included);
    bounded_output(output)
}

fn resolve_linkage(record: &mut Value, identities: &HashSet<(String, String)>) {
    let Some(relationships) = record
        .get_mut("relationships")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    let resolve = |link: &mut Value| {
        if let (Some(kind), Some(id)) = (link["type"].as_str(), link["id"].as_str())
            && identities.contains(&(kind.to_owned(), id.to_owned()))
        {
            // true is shorter than the false reserved during byte accounting.
            link["included"] = json!(true);
        }
    };
    for relationship in relationships.values_mut() {
        match relationship.get_mut("data") {
            Some(Value::Array(links)) => {
                for link in links {
                    resolve(link);
                }
            }
            Some(link @ Value::Object(_)) => resolve(link),
            _ => (),
        }
    }
}

/// Parse the raw wire input as a scalar map: duplicate fields must not disappear into Value.
#[cfg(test)]
pub(crate) fn parse_input(raw: &str) -> Result<Value, ProviderError> {
    struct InputVisitor;
    impl<'de> Visitor<'de> for InputVisitor {
        type Value = Value;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an input object")
        }
        fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Value, M::Error> {
            let mut fields = serde_json::Map::new();
            while let Some((key, value)) = map.next_entry::<String, Value>()? {
                if !matches!(value, Value::String(_) | Value::Bool(_)) {
                    return Err(serde::de::Error::custom("invalid scalar"));
                }
                if fields.insert(key, value).is_some() {
                    return Err(serde::de::Error::custom("duplicate input"));
                }
            }
            Ok(Value::Object(fields))
        }
    }
    let mut decoder = serde_json::Deserializer::from_str(raw);
    let input = decoder
        .deserialize_map(InputVisitor)
        .map_err(|_| invalid_household_input())?;
    decoder.end().map_err(|_| invalid_household_input())?;
    Ok(input)
}
