//! Public `/graphql` call-log roots.
//!
//! Reads reuse the same provider search the contact roots use, so a call and
//! a contact from the same query DSL behave alike.

pub mod mutation;
pub mod query;

use crate::graphql::resolvers::tags::domain_types::Tag;
use crate::graphql::types::facts::{instant, integer, text};
use plain_graphql::{Enum, ID, SimpleObject};
use plain_libs::serde_json::Value;
use plain_rs::features::access::AccessFeature;
use plain_rs::features::services::phone_query::CallType;

const FEATURE: AccessFeature = AccessFeature::Calls;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum PhoneNumberType {
    FixedLine,
    Mobile,
    FixedLineOrMobile,
    TollFree,
    PremiumRate,
    SharedCost,
    PersonalNumber,
    Voip,
    Pager,
    Uan,
    Voicemail,
}

impl From<plain_libs::phone::NumberType> for PhoneNumberType {
    fn from(value: plain_libs::phone::NumberType) -> Self {
        match value {
            plain_libs::phone::NumberType::FixedLine => Self::FixedLine,
            plain_libs::phone::NumberType::Mobile => Self::Mobile,
            plain_libs::phone::NumberType::FixedLineOrMobile => Self::FixedLineOrMobile,
            plain_libs::phone::NumberType::TollFree => Self::TollFree,
            plain_libs::phone::NumberType::PremiumRate => Self::PremiumRate,
            plain_libs::phone::NumberType::SharedCost => Self::SharedCost,
            plain_libs::phone::NumberType::PersonalNumber => Self::PersonalNumber,
            plain_libs::phone::NumberType::Voip => Self::Voip,
            plain_libs::phone::NumberType::Pager => Self::Pager,
            plain_libs::phone::NumberType::Uan => Self::Uan,
            plain_libs::phone::NumberType::Voicemail => Self::Voicemail,
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct PhoneGeo {
    pub country: String,
    #[graphql(name = "numberType")]
    pub number_type: PhoneNumberType,
    pub carrier: String,
    pub description: String,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(complex)]
pub struct Call {
    pub id: ID,
    pub number: String,
    pub name: String,
    pub photo: String,
    #[graphql(name = "startedAt")]
    pub started_at: plain_rs::infrastructure::types::Instant,
    #[graphql(name = "durationSec")]
    pub duration_sec: i32,
    pub r#type: CallType,
    #[graphql(name = "accountId")]
    pub account_id: ID,
    pub geo: Option<PhoneGeo>,
}

impl From<plain_libs::phone::Metadata> for PhoneGeo {
    fn from(value: plain_libs::phone::Metadata) -> Self {
        Self {
            country: value.country,
            number_type: value.number_type.into(),
            carrier: value.carrier,
            description: value.description,
        }
    }
}

#[plain_graphql::ComplexObject]
impl Call {
    async fn tags(&self, ctx: &plain_graphql::Context<'_>) -> plain_graphql::Result<Vec<Tag>> {
        crate::graphql::resolvers::tags::loader::load(
            ctx,
            plain_rs::features::enums::DataType::Call,
            self.id.as_str(),
        )
        .await
    }
}

fn call(fact: &Value) -> Call {
    let id = text(fact, "id");
    Call {
        id: ID::from(id),
        number: text(fact, "number"),
        name: text(fact, "name"),
        photo: text(fact, "photoUri"),
        started_at: instant(fact, "startedAt"),
        duration_sec: integer(fact, "durationSec") as i32,
        r#type: CallType::from_android(integer(fact, "type")),
        account_id: ID::from(text(fact, "accountId")),
        geo: None,
    }
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-server/unit/graphql/resolvers/mobile/phone_calls/mod.rs"]
mod tests;
