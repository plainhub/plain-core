use plain_rs::db::Db;
use plain_rs::utils::search_dsl;
use plain_rs::utils::search_dsl::FilterField;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
#[derive(Deserialize, Serialize, Clone, Copy, Debug)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Provider {
    Call,
    Contact,
    Audio,
    Video,
    Image,
    Doc,
    File,
}
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub clauses: Vec<String>,
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ids_column: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trash: Option<bool>,
}
impl Plan {
    pub fn add(&mut self, clause: String, values: Vec<String>) {
        self.clauses.push(clause);
        self.args.extend(values);
    }
    pub fn equal(&mut self, column: &str, value: &str) {
        self.add(format!("{column} = ?"), vec![value.to_owned()]);
    }
    pub fn contains(&mut self, column: &str, value: &str) {
        let value = value
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        self.add(
            format!("{column} LIKE '%' || ? || '%' ESCAPE '\\'"),
            vec![value],
        );
    }
    pub fn ids(&mut self, column: &str, ids: Vec<String>) {
        if !ids.is_empty() {
            self.ids_column = Some(column.to_owned());
            self.ids = ids;
        }
    }
}
pub fn fields(db: &Db, query: &str) -> anyhow::Result<Vec<FilterField>> {
    let mut fields = search_dsl::parse(query);
    let tag_ids = fields
        .iter()
        .filter(|f| f.name == "tag_id")
        .map(|f| f.value.clone())
        .collect::<std::collections::HashSet<_>>();
    fields.retain(|f| f.name != "tag_id");
    if !tag_ids.is_empty() {
        let mut ids = std::collections::BTreeSet::new();
        for id in tag_ids {
            ids.extend(plain_rs::db::tag::keys_for_tag(db, &id)?);
        }
        fields.push(FilterField {
            name: "ids".into(),
            op: "=".into(),
            value: if ids.is_empty() {
                "invalid_ids".into()
            } else {
                ids.into_iter().collect::<Vec<_>>().join(",")
            },
        });
    }
    Ok(fields)
}
/// Names the web client puts in a query that this provider must not act on.
///
/// Every entry is here because a builder in `plain-desktop` can emit it for
/// this provider, and the field has no match arm. `all` and `trash` are
/// accepted everywhere: `all` is the only sanctioned way to ask for a whole
/// table, and `trash` reaches the media providers as a MediaStore query
/// argument, which Contacts and Calls have no store to answer.
pub fn intentionally_ignored(provider: Provider, field: &str) -> bool {
    match (provider, field) {
        (_, "all" | "trash") => true,
        // Read before the loop, where it decides whether dotfiles are hidden.
        // The docs view shares the files filter object, so it arrives there too.
        (_, "show_hidden") => true,
        // Sent when a mount or a favorite folder is picked in the files
        // sidebar; the listing itself is scoped by `parent`, not by this.
        (Provider::File | Provider::Doc, "root_path") => true,
        // The apps and messages views are the only ones that set `type`, and
        // neither branch clears it when the user navigates away — so a media
        // search made after picking `type:USER` in Apps carries it over. A media
        // provider's own kind is already the provider choice, so there is no
        // column left for it to compare against.
        (Provider::Audio | Provider::Video | Provider::Image, "type") => true,
        _ => false,
    }
}
pub fn comparison(field: &FilterField) -> (&str, &str) {
    let mut value = field.value.trim();
    let mut op = field.op.as_str();
    if matches!(op, "" | ":" | "=") {
        op = "=";
        for prefix in [">=", "<=", "!=", ">", "<", "="] {
            if let Some(rest) = value.strip_prefix(prefix) {
                op = prefix;
                value = rest.trim();
                break;
            }
        }
    } else if let Some(rest) = value.strip_prefix(op) {
        value = rest.trim();
    }
    (op, value)
}
pub fn plan(
    provider: Provider,
    fields: &[FilterField],
    _dates: &Value,
    resolved_parent_id: Option<&str>,
    query_is_empty: bool,
) -> anyhow::Result<Plan> {
    let mut plan = Plan::default();

    if matches!(provider, Provider::Doc) {
        plan.add(
            format!(
                "(mime_type LIKE ? OR mime_type IN ({}))",
                vec!["?"; plain_rs::media::documents::MIME_TYPES.len()].join(",")
            ),
            std::iter::once("text/%".to_owned())
                .chain(
                    plain_rs::media::documents::MIME_TYPES
                        .iter()
                        .map(|value| (*value).to_owned()),
                )
                .collect(),
        );
        plan.add("_size > 0".into(), vec![]);
    }
    if matches!(provider, Provider::File) && !query_is_empty {
        let show_hidden = fields
            .iter()
            .any(|field| field.name == "show_hidden" && field.value.eq_ignore_ascii_case("true"));
        if !show_hidden {
            plan.add(
                "_display_name NOT LIKE ? ESCAPE '\\'".into(),
                vec![".%".into()],
            );
        }
    }
    for field in fields {
        match (provider, field.name.as_str()) {
            (Provider::Audio, "text") => plan.add("(title LIKE '%' || ? || '%' ESCAPE '\\' OR artist LIKE '%' || ? || '%' ESCAPE '\\')".into(), vec![escape_like(&field.value), escape_like(&field.value)]),
            (Provider::Audio, "path") => plan.equal("_data", &field.value),
            (Provider::Audio, "name") => plan.equal("title", &field.value),
            (Provider::Audio, "artist") => plan.equal("artist", &field.value),
            (Provider::Video | Provider::Image, "text") => {
                let column = if matches!(provider, Provider::Image) { "(title LIKE '%' || ? || '%' ESCAPE '\\' OR _data LIKE '%' || ? || '%' ESCAPE '\\')" } else { "title LIKE '%' || ? || '%' ESCAPE '\\'" };
                let value = escape_like(&field.value);
                plan.add(column.into(), if matches!(provider, Provider::Image) { vec![value.clone(), value] } else { vec![value] });
            }
            (Provider::Audio | Provider::Video | Provider::Image | Provider::Doc, "ids") => plan.ids("_id", field.value.split(',').map(str::to_owned).collect()),
            (Provider::Audio | Provider::Video | Provider::Image | Provider::Doc, "bucket_id") => plan.equal("bucket_id", &field.value),
            (Provider::Audio | Provider::Video | Provider::Image | Provider::Doc, "excluded_dir") => plan.add("_data NOT LIKE ? || '%' ESCAPE '\\'".into(), vec![escape_like(&field.value)]),
            (Provider::Doc, "text") => plan.add("_display_name LIKE '%' || ? || '%' ESCAPE '\\'".into(), vec![escape_like(&field.value)]),
            (Provider::Doc, "ext") => plan.add("_display_name LIKE ? ESCAPE '\\'".into(), vec![format!("%.{}", escape_like(&field.value))]),
            (Provider::Doc, "parent") => plan.add("_data LIKE ? ESCAPE '\\'".into(), vec![format!("{}/%", escape_like(field.value.trim_end_matches('/')))]),
            (Provider::Doc, "type") => plan.equal("mime_type", &field.value),
            (Provider::File | Provider::Doc | Provider::Audio | Provider::Video | Provider::Image, "file_size" | "size") => {
                let (op, value) = comparison(field);
                if ["=", "!=", ">", ">=", "<", "<="].contains(&op) {
                    if let Some(bytes) = parse_size_to_bytes(value) { plan.add(format!("_size {op} ?"), vec![bytes.to_string()]); }
                }
            }
            (Provider::File, "text") => plan.add("_display_name LIKE '%' || ? || '%' ESCAPE '\\'".into(), vec![escape_like(&field.value)]),
            (Provider::File, "parent") => plan.add("parent = ?".into(), vec![resolved_parent_id.unwrap_or("-1").to_owned()]),
            (Provider::File, "type") => plan.equal("mime_type", &field.value),
            (Provider::File, "ids") => plan.ids("_id", field.value.split(',').map(str::to_owned).collect()),
            (Provider::Audio | Provider::Video | Provider::Image | Provider::Doc, "trash") => {
                if let Some(flag) = field.value.parse::<bool>().ok() {
                    plan.trash = Some(flag);
                }
            }
            // Reached only when no arm above matched, so these are fields this
            // provider has no column for. Dropping them used to leave the
            // clause list empty — which ContentWhere renders as `1=1`, turning
            // a delete into a whole-table operation. Say which field instead.
            _ => {
                if !intentionally_ignored(provider, field.name.as_str()) {
                    return Err(anyhow::anyhow!(
                        "unsupported {provider:?} filter: {}",
                        field.name
                    ));
                }
            }
        }
    }
    Ok(plan)
}
fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}
fn parse_size_to_bytes(value: &str) -> Option<u64> {
    let value = value.trim();
    let split = value
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(value.len());
    let number = value[..split].parse::<f64>().ok()?;
    let unit = value[split..].trim().to_ascii_lowercase();
    let multiplier = match unit.as_str() {
        "" | "b" => 1.0,
        "kb" | "kib" => 1024.0,
        "mb" | "mib" => 1024.0_f64.powi(2),
        "gb" | "gib" => 1024.0_f64.powi(3),
        "tb" | "tib" => 1024.0_f64.powi(4),
        _ => return None,
    };
    let bytes = number * multiplier;
    (bytes.is_finite() && bytes >= 0.0 && bytes <= u64::MAX as f64).then_some(bytes as u64)
}
pub fn prepare(
    db: &Db,
    provider: Provider,
    query: &str,
    resolved_parent_id: Option<&str>,
) -> anyhow::Result<Plan> {
    let fields = fields(db, query)?;
    let dates = dates(&fields);
    if matches!(provider, Provider::Call | Provider::Contact) {
        return plain_rs::phone::query::plan(provider, &fields, &dates);
    }
    plan(
        provider,
        &fields,
        &dates,
        resolved_parent_id,
        query.is_empty(),
    )
}

pub fn params(
    db: &Db,
    provider: Provider,
    query: &str,
    mut params: Value,
) -> anyhow::Result<Value> {
    params["plan"] = serde_json::to_value(prepare(db, provider, query, None)?)?;
    Ok(params)
}

pub fn media_provider(kind: plain_rs::enums::DataType) -> anyhow::Result<Provider> {
    use plain_rs::enums::DataType;
    match kind {
        DataType::Audio => Ok(Provider::Audio),
        DataType::Video => Ok(Provider::Video),
        DataType::Image => Ok(Provider::Image),
        DataType::Doc => Ok(Provider::Doc),
        DataType::Call => Ok(Provider::Call),
        DataType::Contact => Ok(Provider::Contact),
        _ => anyhow::bail!("unsupported provider: {kind:?}"),
    }
}

pub fn dates(fields: &[FilterField]) -> Value {
    let values: serde_json::Map<String, Value> = fields
        .iter()
        .filter(|field| field.name == "start_time")
        .map(|field| {
            let value = comparison(field).1;
            (
                value.to_owned(),
                json!(plain_rs::utils::epoch_millis::parse(value)),
            )
        })
        .collect();
    if values.is_empty() {
        Value::Null
    } else {
        Value::Object(values)
    }
}

pub fn require_narrowing_with(
    builder: impl Fn(Provider, &[FilterField], &Value, Option<&str>, bool) -> anyhow::Result<Plan>,
    db: &Db,
    provider: Provider,
    query: &str,
) -> anyhow::Result<()> {
    // A blank query is the caller's own guard to own, and `all` builds no
    // clause on purpose: it is how a caller says "every row, I mean it".
    if query.trim().is_empty() {
        return Ok(());
    }
    let parsed = fields(db, query)?;
    if parsed
        .iter()
        .any(|f| f.name == "all" && f.value.eq_ignore_ascii_case("true"))
    {
        return Ok(());
    }
    let plan = builder(provider, &parsed, &dates(&parsed), None, query.is_empty())?;
    if plan.trash == Some(true) {
        return Ok(());
    }
    let baseline = builder(provider, &[], &Value::Null, None, false)?;
    if plan.clauses.len() <= baseline.clauses.len() && plan.ids.is_empty() {
        return Err(anyhow::anyhow!(
            "query {query:?} selects no {provider:?} row — nothing in it narrows the selection, so \
             the operation would cover the whole table; pass 'all:true' to target every row on \
             purpose"
        ));
    }
    Ok(())
}
pub fn require_narrowing(db: &Db, provider: Provider, query: &str) -> anyhow::Result<()> {
    require_narrowing_with(plan, db, provider, query)
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/provider_query.rs"]
mod tests;
