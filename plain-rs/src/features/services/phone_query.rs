use crate::features::services::files::provider_query::Plan;
use crate::features::services::files::provider_query::Provider;
use crate::features::services::files::provider_query::comparison;
use crate::features::services::files::provider_query::intentionally_ignored;
use plain_libs::utils::search_dsl::FilterField;
use plain_libs::serde_json::Value;
macro_rules! provider_type {
    ($name:ident { $($variant:ident = $code:literal),+ }) => {
        #[derive(plain_graphql::Enum, serde::Deserialize, Copy, Clone, Eq, PartialEq, Debug, Default)]
        #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
        pub enum $name {
            $($variant = $code,)+
            #[default]
            Unknown,
        }

        impl $name {
            pub fn from_android(value: i64) -> Self {
                match value {
                    $($code => Self::$variant,)+
                    _ => Self::Unknown,
                }
            }

            pub(super) fn filter(self, plan: &mut Plan, column: &str) {
                if self == Self::Unknown {
                    plan.add(format!("{column} NOT IN ({})", [$(stringify!($code)),+].join(",")), vec![]);
                } else {
                    plan.equal(column, &(self as i64).to_string());
                }
            }
        }
    };
}

provider_type!(SmsType {
    Inbox = 1,
    Sent = 2,
    Draft = 3,
    Outbox = 4,
    Failed = 5,
    Queued = 6
});

provider_type!(CallType {
    Incoming = 1,
    Outgoing = 2,
    Missed = 3,
    Voicemail = 4,
    Rejected = 5,
    Blocked = 6,
    AnsweredExternally = 7
});

pub fn plan(provider: Provider, fields: &[FilterField], dates: &Value) -> plain_libs::anyhow::Result<Plan> {
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
            (Provider::Call, "type") => {
                let kind: CallType = serde::Deserialize::deserialize(Value::String(field.value.clone()))?;
                kind.filter(&mut plan, "type");
            }
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
                    plain_libs::anyhow::bail!("unsupported {provider:?} filter: {}", field.name);
                }
            }
        }
    }
    Ok(plan)
}

pub fn require_narrowing(
    db: &crate::infrastructure::db::Db,
    provider: Provider,
    query: &str,
) -> plain_libs::anyhow::Result<()> {
    crate::features::services::files::provider_query::require_narrowing_with(
        |provider, fields, dates, _, _| plan(provider, fields, dates),
        db,
        provider,
        query,
    )
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/services/phone_query.rs"]
mod tests;
