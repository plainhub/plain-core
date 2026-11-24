use crate::graphql::resolvers::mobile::contacts::query::ContactsQuery;
use crate::graphql::resolvers::mobile::phone_calls::query::CallsQuery;
use crate::graphql::resolvers::packages::query::PackagesQuery;
use crate::graphql::resolvers::sms::query::SmsQuery;
use plain_graphql::MergedObject;

// The merged object derives its GraphQL name from its Rust type.
#[derive(MergedObject, Default)]
pub struct MobileQuery(PackagesQuery, ContactsQuery, SmsQuery, CallsQuery);
