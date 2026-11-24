use super::{FEATURE, contact, contact_rows};
use crate::graphql::resolvers::mobile::contacts::types::*;
use crate::graphql::types::ActionResult;
use crate::graphql::types::facts::{host_json, provider_params, text};
use crate::http::auth::public_gate;
use plain_graphql::{Context, ID, Object};
use plain_libs::serde_json::{Value, json};
use plain_rs::features::capability::Capability;
use plain_rs::features::prefs::Prefs;
use plain_rs::features::services::capabilities;
use plain_rs::features::services::contacts::write as contact_write;
use plain_rs::features::services::contacts::write::Action;
use plain_rs::infrastructure::host::bridge::Host;
use std::sync::Arc;

#[derive(Default)]
pub struct ContactsMutation;

#[Object]
impl ContactsMutation {
    async fn create_contact(
        &self,
        ctx: &Context<'_>,
        input: ContactInput,
    ) -> plain_graphql::Result<Contact> {
        let id = contact_write_call(
            ctx,
            contact_write::Request {
                action: Action::CreateContact,
                id: None,
                input: Some(contact_input(&input)),
                name: None,
                account_name: None,
                account_type: None,
            },
        )
        .await?;
        reload(ctx, &id, "after create").await
    }

    async fn update_contact(
        &self,
        ctx: &Context<'_>,
        id: ID,
        input: ContactInput,
    ) -> plain_graphql::Result<Contact> {
        contact_write_call(
            ctx,
            contact_write::Request {
                action: Action::UpdateContact,
                id: Some(id.0.clone()),
                input: Some(contact_input(&input)),
                name: None,
                account_name: None,
                account_type: None,
            },
        )
        .await?;
        reload(ctx, &id.0, "after update").await
    }

    async fn delete_contacts(
        &self,
        ctx: &Context<'_>,
        query: String,
    ) -> plain_graphql::Result<ActionResult> {
        if query.trim().is_empty() {
            return Err(plain_graphql::Error::new(
                "query is required for bulk mutations — pass 'all:true' to explicitly target everything",
            ));
        }
        public_gate::require(ctx, ctx.data_unchecked::<Arc<Prefs>>(), &[FEATURE])?;
        capabilities::require(ctx.data_unchecked::<Arc<Host>>(), Capability::Telephony)
            .await
            .map_err(|error| plain_graphql::Error::from(error))?;
        plain_rs::features::services::phone_query::require_narrowing(
            ctx.data_unchecked::<Arc<plain_rs::infrastructure::db::Db>>(),
            plain_rs::features::services::files::provider_query::Provider::Contact,
            &query,
        )
        .map_err(|error| plain_graphql::Error::from(error))?;
        let ids: Vec<Value> = host_json(
            ctx,
            "systemContactIds",
            provider_params(
                ctx,
                plain_rs::features::services::files::provider_query::Provider::Contact,
                &query,
                plain_libs::serde_json::json!({}),
            )
            .await?,
        )
        .await?
        .iter()
        .filter_map(Value::as_str)
        .map(Value::from)
        .collect();
        let deleted: Vec<Value> = plain_libs::serde_json::from_value(
            ctx.data_unchecked::<Arc<Host>>()
                .call(
                    "systemDeleteRecords",
                    json!({"provider": "CONTACT", "ids": ids}),
                )
                .await
                .map_err(|error| plain_graphql::Error::new(error))?,
        )
        .map_err(|error| plain_graphql::Error::from(error))?;
        Ok(ActionResult {
            affected_count: deleted.len() as i32,
        })
    }

    async fn create_contact_group(
        &self,
        ctx: &Context<'_>,
        name: String,
        account_name: String,
        account_type: String,
    ) -> plain_graphql::Result<ContactGroup> {
        let created = contact_write_call(
            ctx,
            contact_write::Request {
                action: Action::CreateGroup,
                id: None,
                input: None,
                name: Some(name.clone()),
                account_name: Some(account_name),
                account_type: Some(account_type),
            },
        )
        .await?;
        Ok(ContactGroup {
            id: ID::from(created),
            name,
            contact_count: 0,
        })
    }

    async fn update_contact_group(
        &self,
        ctx: &Context<'_>,
        id: ID,
        name: String,
    ) -> plain_graphql::Result<ContactGroup> {
        contact_write_call(
            ctx,
            contact_write::Request {
                action: Action::UpdateGroup,
                id: Some(id.0.clone()),
                input: None,
                name: Some(name.clone()),
                account_name: None,
                account_type: None,
            },
        )
        .await?;
        Ok(ContactGroup {
            id,
            name,
            contact_count: 0,
        })
    }

    async fn delete_contact_group(&self, ctx: &Context<'_>, id: ID) -> plain_graphql::Result<bool> {
        contact_write_call(
            ctx,
            contact_write::Request {
                action: Action::DeleteGroup,
                id: Some(id.0),
                input: None,
                name: None,
                account_name: None,
                account_type: None,
            },
        )
        .await?;
        Ok(true)
    }
}

/// Returns the id the platform assigned to a newly created row; every other
/// action comes back without one.
async fn contact_write_call(
    ctx: &Context<'_>,
    request: contact_write::Request,
) -> plain_graphql::Result<String> {
    let result = contact_write::execute(
        ctx.data_unchecked::<Arc<Prefs>>(),
        ctx.data_unchecked::<Arc<Host>>(),
        request,
    )
    .await
    .map_err(|error| plain_graphql::Error::from(error))?;
    Ok(result["id"].as_str().unwrap_or_default().to_owned())
}

async fn reload(ctx: &Context<'_>, id: &str, phase: &str) -> plain_graphql::Result<Contact> {
    let facts = contact_rows(
        ctx,
        provider_params(
            ctx,
            plain_rs::features::services::files::provider_query::Provider::Contact,
            &format!("id:{id}"),
            json!({"offset": 0, "limit": 1}),
        )
        .await?,
    )
    .await?;
    let fact = facts
        .iter()
        .find(|fact| text(fact, "id") == id)
        .ok_or_else(|| plain_graphql::Error::new(format!("Contact {id} not found {phase}")))?;
    Ok(contact(fact))
}

/// The payload shape the platform's `ContactInput` deserializes.
fn contact_input(input: &ContactInput) -> Value {
    json!({
        "prefix": input.prefix,
        "firstName": input.first_name,
        "middleName": input.middle_name,
        "lastName": input.last_name,
        "suffix": input.suffix,
        "nickname": input.nickname,
        "phoneNumbers": input.phone_numbers.iter().map(|item| json!({
            "value": item.value, "type": item.r#type.name(), "label": item.label,
        })).collect::<Vec<_>>(),
        "emails": input.emails.iter().map(|item| json!({
            "value": item.value, "type": item.r#type.name(), "label": item.label,
        })).collect::<Vec<_>>(),
        "addresses": input.addresses.iter().map(|item| json!({
            "value": item.value, "type": item.r#type.name(), "label": item.label,
        })).collect::<Vec<_>>(),
        "events": input.events.iter().map(|item| json!({
            "value": item.value, "type": item.r#type.name(), "label": item.label,
        })).collect::<Vec<_>>(),
        "websites": input.websites.iter().map(|item| json!({
            "value": item.value, "type": item.r#type.name(), "label": item.label,
        })).collect::<Vec<_>>(),
        "ims": input.ims.iter().map(|item| json!({
            "value": item.value, "protocol": item.protocol.name(), "customProtocol": item.custom_protocol,
        })).collect::<Vec<_>>(),
        "source": input.source,
        "starred": input.starred,
        "notes": input.notes,
        "groupIds": input.group_ids.iter().map(|id| Value::from(id.0.clone())).collect::<Vec<_>>(),
        "organization": input.organization.as_ref().map(|org| json!({
            "company": org.company, "title": org.title,
        })),
    })
}
