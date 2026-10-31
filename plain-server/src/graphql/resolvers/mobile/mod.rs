pub mod contacts;

pub mod phone_calls;

use crate::graphql::resolvers::mobile::contacts::resolvers::ContactsMutation;
use crate::graphql::resolvers::mobile::contacts::resolvers::ContactsQuery;
use crate::graphql::resolvers::mobile::phone_calls::resolvers::CallsMutation;
use crate::graphql::resolvers::mobile::phone_calls::resolvers::CallsQuery;
use crate::graphql::resolvers::packages::public::PackagesMutation;
use crate::graphql::resolvers::packages::public::PackagesQuery;
use crate::graphql::resolvers::sms::SmsMutation;
use crate::graphql::resolvers::sms::SmsQuery;
use async_graphql::MergedObject;
// Not named `Query`: async-graphql derives a merged object's GraphQL name
// from its Rust type name, and this one is merged into `public::Query`.
#[derive(MergedObject, Default)]
pub struct MobileQuery(PackagesQuery, ContactsQuery, SmsQuery, CallsQuery);
// Not named `Mutation`, for the same reason as `MobileQuery`.
#[derive(MergedObject, Default)]
pub struct MobileMutation(
    PackagesMutation,
    ContactsMutation,
    SmsMutation,
    CallsMutation,
);
