pub mod mutation;
pub mod types;

pub mod query;

use crate::graphql::resolvers::mobile::contacts::types::*;
use crate::graphql::types::facts::{instant, integer, list, text};
use plain_graphql::{Context, ID};
use plain_libs::serde_json::Value;
use plain_rs::features::access::AccessFeature;
use plain_rs::infrastructure::host::bridge::Host;
use std::sync::Arc;

const FEATURE: AccessFeature = AccessFeature::Contacts;

/// The contract returns the freshly written contact, so the row is read back
/// through the same provider search instead of being assembled from the input.
async fn contact_rows(ctx: &Context<'_>, params: Value) -> plain_graphql::Result<Vec<Value>> {
    let value = ctx
        .data_unchecked::<Arc<Host>>()
        .call("systemContactFacts", params)
        .await
        .map_err(plain_graphql::Error::new)?;
    plain_rs::features::services::contacts::projection::contacts(value)
        .map_err(|error| plain_graphql::Error::from(error))
}

fn contact(fact: &Value) -> Contact {
    let id = text(fact, "id");
    Contact {
        id: ID::from(id),
        prefix: text(fact, "prefix"),
        first_name: text(fact, "firstName"),
        middle_name: text(fact, "middleName"),
        last_name: text(fact, "lastName"),
        suffix: text(fact, "suffix"),
        nickname: text(fact, "nickname"),
        photo_id: text(fact, "photoId"),
        phone_numbers: list(fact, "phoneNumbers", |item| ContactPhoneNumber {
            value: text(item, "value"),
            r#type: ContactPhoneType::from_android(integer(item, "type")),
            label: text(item, "label"),
            normalized_number: text(item, "normalizedNumber"),
        }),
        emails: list(fact, "emails", |item| ContactEmail {
            value: text(item, "value"),
            r#type: ContactEmailType::from_android(integer(item, "type")),
            label: text(item, "label"),
        }),
        addresses: list(fact, "addresses", |item| ContactAddress {
            value: text(item, "value"),
            r#type: ContactPostalType::from_android(integer(item, "type")),
            label: text(item, "label"),
        }),
        events: list(fact, "events", |item| ContactEvent {
            value: text(item, "value"),
            r#type: ContactEventType::from_android(integer(item, "type")),
            label: text(item, "label"),
        }),
        websites: list(fact, "websites", |item| ContactWebsite {
            value: text(item, "value"),
            r#type: ContactWebsiteType::from_android(integer(item, "type")),
            label: text(item, "label"),
        }),
        ims: list(fact, "ims", |item| ContactIm {
            value: text(item, "value"),
            protocol: ContactImProtocol::from_android(integer(item, "type")),
            custom_protocol: text(item, "label"),
        }),
        source: text(fact, "source"),
        starred: fact["starred"].as_bool().unwrap_or(false),
        contact_id: ID::from(text(fact, "contactId")),
        thumbnail_id: text(fact, "thumbnailId"),
        notes: text(fact, "notes"),
        groups: list(fact, "groups", group),
        organization: organization(fact),
        ringtone: text(fact, "ringtone"),
        updated_at: instant(fact, "updatedAt"),
    }
}

fn organization(fact: &Value) -> Option<ContactOrganization> {
    let org = fact["organization"].as_object()?;
    Some(ContactOrganization {
        company: text(&Value::Object(org.clone()), "company"),
        title: text(&Value::Object(org.clone()), "title"),
    })
}

fn group(fact: &Value) -> ContactGroup {
    ContactGroup {
        id: ID::from(text(fact, "id")),
        name: text(fact, "name"),
        contact_count: 0,
    }
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-server/unit/graphql/resolvers/mobile/contacts/mod.rs"]
mod tests;
