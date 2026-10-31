use plain_rs::services::files::provider_query::Plan;
use plain_rs::services::files::provider_query::Provider;
use plain_rs::services::files::provider_query::comparison;
use plain_rs::services::files::provider_query::intentionally_ignored;
use plain_rs::utils::search_dsl::FilterField;
use serde_json::Value;
pub fn plan(provider: Provider, fields: &[FilterField], dates: &Value) -> anyhow::Result<Plan> {
    let mut plan = Plan::default();
    if matches!(provider, Provider::Contact) {
        plan.equal("mimetype", "vnd.android.cursor.item/name");
    }
    for field in fields {
        match (provider, field.name.as_str()) {
            (Provider::Call, "text") => plan.contains("number", &field.value),
            (Provider::Contact, "text") => plan.contains("data2", &field.value),
            (Provider::Call, "ids") => {
                plan.ids("_id", field.value.split(',').map(str::to_owned).collect())
            }
            (Provider::Contact, "ids") => plan.ids(
                "raw_contact_id",
                field.value.split(',').map(str::to_owned).collect(),
            ),
            (Provider::Contact, "id") => plan.equal("raw_contact_id", &field.value),
            (Provider::Call, "type") => plan.equal("type", &field.value),
            (Provider::Call, "duration") | (Provider::Call, "start_time") => {
                let (op, value) = comparison(field);
                if !["=", "!=", ">", ">=", "<", "<="].contains(&op) {
                    continue;
                }
                let (column, number) = if field.name == "duration" {
                    ("duration", value.parse::<i64>().ok())
                } else {
                    ("date", dates[value].as_i64())
                };
                if let Some(number) = number {
                    plan.add(format!("{column} {op} ?"), vec![number.to_string()]);
                }
            }
            _ => {
                if !intentionally_ignored(provider, field.name.as_str()) {
                    anyhow::bail!("unsupported {provider:?} filter: {}", field.name);
                }
            }
        }
    }
    Ok(plan)
}

pub fn require_narrowing(
    db: &plain_rs::db::Db,
    provider: Provider,
    query: &str,
) -> anyhow::Result<()> {
    plain_rs::services::files::provider_query::require_narrowing_with(
        |provider, fields, dates, _, _| plan(provider, fields, dates),
        db,
        provider,
        query,
    )
}

#[cfg(test)]
#[path = "../../tests/unit/phone/query.rs"]
mod tests;
