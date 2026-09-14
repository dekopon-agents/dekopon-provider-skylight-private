//! Source-backed bounded reads, not a Tasks decoder or an identity resolver.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Read {
    Categories,
    Events,
    Lists,
    List,
    Items,
}

pub(crate) const READS: [Read; 5] = [
    Read::Categories,
    Read::Events,
    Read::Lists,
    Read::List,
    Read::Items,
];
const MAX_RECORDS: usize = 64;

impl Read {
    pub(crate) fn capability(self) -> &'static str {
        match self {
            Self::Categories => "skylight.private.categories.list",
            Self::Events => "skylight.private.calendar.events.list",
            Self::Lists => "skylight.private.lists.list",
            Self::List => "skylight.private.lists.read",
            Self::Items => "skylight.private.list.items.list",
        }
    }

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
            ],
        }
    }

    pub(crate) fn manifest(self) -> ProviderCapability {
        let mut properties = serde_json::Map::new();
        for (_, field) in self.fields() {
            let schema = match *field {
                "frameId" | "listId" => {
                    json!({"type":"string", "minLength":1, "maxLength":128, "pattern":"^[A-Za-z0-9_-]+$"})
                }
                "dateMin" | "dateMax" => {
                    json!({"type":"string", "format":"date", "pattern":"^[0-9]{4}-[0-9]{2}-[0-9]{2}$"})
                }
                _ => {
                    json!({"type":"string", "minLength":1, "maxLength":128, "pattern":"^[A-Za-z0-9_+/-]+$"})
                }
            };
            properties.insert((*field).to_owned(), schema);
        }
        ProviderCapability {
            id: self.capability().parse().expect("static capability"),
            description: match self {
                Self::Categories => {
                    "Lists bounded category identifiers and labels; not person identities"
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
            input_schema: json!({"type":"object", "properties":properties, "required":self.fields().iter().map(|(_, key)| *key).collect::<Vec<_>>(), "additionalProperties":false}),
        }
    }

    pub(crate) fn uri(self, input: &Value) -> Result<String, ProviderError> {
        let object = input.as_object().ok_or_else(invalid_household_input)?;
        if object.len() != self.fields().len()
            || self
                .fields()
                .iter()
                .any(|(_, field)| !object.contains_key(*field))
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
                format!(
                    "{base}/calendar_events?date_min={}&date_max={}&timezone={}",
                    encode_query(&format!("{from}T00:00:00")),
                    encode_query(&format!("{to}T00:00:00")),
                    encode_query(timezone)
                )
            }
        })
    }

    pub(crate) fn invoke<F>(self, input: Value, send: F) -> Result<Value, ProviderError>
    where
        F: FnOnce(Request) -> Result<Response, HttpError>,
    {
        let body = send_once(&self.uri(&input)?, send)?;
        match self {
            Self::Categories => project::<LabelAttributes>(&body, "categories", false),
            Self::Events => project::<EventAttributes>(&body, "events", false),
            Self::Lists => project::<LabelAttributes>(&body, "lists", false),
            Self::List => project::<LabelAttributes>(&body, "list", true),
            Self::Items => project::<ItemAttributes>(&body, "items", false),
        }
    }
}

pub(crate) fn invalid_household_input() -> ProviderError {
    ProviderError::new(
        "invalid-input",
        "input must match the bounded Skylight read schema",
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
#[derive(Debug, Default, Deserialize)]
#[serde(transparent)]
struct Nullable<T>(Option<T>);

#[derive(Default, Deserialize)]
struct LabelAttributes {
    #[serde(default)]
    label: Nullable<String>,
}
#[derive(Default, Deserialize)]
struct EventAttributes {
    #[serde(default)]
    summary: Nullable<String>,
    #[serde(default)]
    starts_at: Nullable<String>,
    #[serde(default)]
    ends_at: Nullable<String>,
    #[serde(default)]
    all_day: Nullable<bool>,
}
#[derive(Default, Deserialize)]
struct ItemAttributes {
    #[serde(default)]
    label: Nullable<String>,
    #[serde(default)]
    status: Nullable<String>,
    #[serde(default)]
    section: Nullable<String>,
}

trait Projection {
    fn validate(&self) -> Result<bool, ProviderError>;
    fn fields(self) -> Result<Value, ProviderError>;
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
impl Projection for LabelAttributes {
    fn validate(&self) -> Result<bool, ProviderError> {
        Ok(self
            .label
            .0
            .as_ref()
            .is_some_and(|v| v.len() > MAX_NAME_BYTES))
    }
    fn fields(self) -> Result<Value, ProviderError> {
        let mut output = json!({});
        let truncated = text(&mut output, "label", self.label);
        output["textTruncated"] = json!(truncated);
        Ok(output)
    }
}
impl Projection for EventAttributes {
    fn validate(&self) -> Result<bool, ProviderError> {
        for value in [&self.starts_at, &self.ends_at] {
            if value.0.as_ref().is_some_and(|v| v.len() > MAX_NAME_BYTES) {
                return Err(invalid_response());
            }
        }
        Ok(self
            .summary
            .0
            .as_ref()
            .is_some_and(|v| v.len() > MAX_NAME_BYTES))
    }
    fn fields(self) -> Result<Value, ProviderError> {
        let mut output = json!({"allDay":self.all_day.0});
        let truncated = text(&mut output, "summary", self.summary);
        // Times are opaque source values, not parsed, converted or truncated into misleading times.
        for (key, value) in [("startsAt", self.starts_at), ("endsAt", self.ends_at)] {
            if value.0.as_ref().is_some_and(|v| v.len() > MAX_NAME_BYTES) {
                return Err(invalid_response());
            }
            output[key] = json!(value.0);
        }
        output["textTruncated"] = json!(truncated);
        Ok(output)
    }
}
impl Projection for ItemAttributes {
    fn validate(&self) -> Result<bool, ProviderError> {
        Ok([&self.label, &self.section]
            .iter()
            .any(|v| v.0.as_ref().is_some_and(|s| s.len() > MAX_NAME_BYTES)))
    }
    fn fields(self) -> Result<Value, ProviderError> {
        let mut output = json!({});
        let mut truncated = text(&mut output, "label", self.label);
        truncated |= text(&mut output, "section", self.section);
        // Only the two source-backed tokens have completion meaning; no outstanding inference.
        output["status"] = match self.status.0.as_deref() {
            Some("pending") => json!("pending"),
            Some("completed") => json!("completed"),
            _ => Value::Null,
        };
        output["textTruncated"] = json!(truncated);
        Ok(output)
    }
}

#[derive(Deserialize)]
#[serde(bound(deserialize = "A: Deserialize<'de> + Default"))]
struct Resource<A> {
    id: String,
    #[serde(default)]
    attributes: JsonObject<A>,
}
#[derive(Deserialize)]
struct Envelope<T> {
    data: T,
}

struct Resources<A> {
    selected: BTreeMap<String, A>,
    total: usize,
    text_truncated: bool,
}
impl<'de, A: Deserialize<'de> + Default + Projection> Deserialize<'de> for Resources<A> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ResourcesVisitor<A>(PhantomData<A>);
        impl<'de, A: Deserialize<'de> + Default + Projection> Visitor<'de> for ResourcesVisitor<A> {
            type Value = Resources<A>;
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
                while let Some(JsonObject(resource)) =
                    seq.next_element::<JsonObject<Resource<A>>>()?
                {
                    validate_id(&resource.id).map_err(serde::de::Error::custom)?;
                    if !seen.insert(resource.id.clone()) {
                        return Err(serde::de::Error::custom("duplicate identifier"));
                    }
                    // Validate every record before local limits, but retain typed attributes.
                    // Project only the final selection: descending IDs replace every retained
                    // record and must not cause per-input-record JSON allocation.
                    result.text_truncated |= resource
                        .attributes
                        .0
                        .validate()
                        .map_err(serde::de::Error::custom)?;
                    result.total += 1;
                    if result.selected.len() < MAX_RECORDS
                        || result
                            .selected
                            .last_key_value()
                            .is_some_and(|(id, _)| resource.id < *id)
                    {
                        result.selected.insert(resource.id, resource.attributes.0);
                        if result.selected.len() > MAX_RECORDS {
                            result.selected.pop_last();
                        }
                    }
                }
                Ok(result)
            }
        }
        deserializer.deserialize_seq(ResourcesVisitor(PhantomData))
    }
}
fn project<A>(body: &[u8], key: &str, single: bool) -> Result<Value, ProviderError>
where
    A: for<'de> Deserialize<'de> + Default + Projection,
{
    if single {
        let envelope: JsonObject<Envelope<JsonObject<Resource<A>>>> =
            serde_json::from_slice(body).map_err(|_| invalid_response())?;
        let resource = envelope.0.data.0;
        validate_id(&resource.id)?;
        resource.attributes.0.validate()?;
        let mut record = resource.attributes.0.fields()?;
        record["id"] = json!(resource.id);
        return bounded_output(
            json!({key:record, "truncated":record["textTruncated"], "coverage":"bounded-response", "upstreamCompleteness":"unknown"}),
        );
    }
    let envelope: JsonObject<Envelope<Resources<A>>> =
        serde_json::from_slice(body).map_err(|_| invalid_response())?;
    let resources = envelope.0.data;
    let mut records = resources
        .selected
        .into_iter()
        .map(|(id, attributes)| {
            let mut output = attributes.fields()?;
            output["id"] = json!(id);
            Ok(output)
        })
        .collect::<Result<Vec<_>, ProviderError>>()?;
    loop {
        let output = json!({key:records, "truncated":resources.text_truncated || records.len() < resources.total, "coverage":"bounded-response", "upstreamCompleteness":"unknown"});
        if serde_json::to_vec(&output)
            .map_err(|_| invalid_response())?
            .len()
            <= MAX_PROJECTED_OUTPUT_BYTES
        {
            return Ok(output);
        }
        records.pop().ok_or_else(invalid_response)?;
    }
}

/// Parse the raw wire input as a string map: duplicate fields must not disappear into Value.
pub(crate) fn parse_input(raw: &str) -> Result<Value, ProviderError> {
    struct InputVisitor;
    impl<'de> Visitor<'de> for InputVisitor {
        type Value = Value;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an input object")
        }
        fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Value, M::Error> {
            let mut fields = serde_json::Map::new();
            while let Some((key, value)) = map.next_entry::<String, String>()? {
                if fields.insert(key, Value::String(value)).is_some() {
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
