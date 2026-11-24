use crate::graphql::resolvers::mobile::contacts::mutation::ContactsMutation;
use crate::graphql::resolvers::mobile::phone_calls::mutation::CallsMutation;
use crate::graphql::resolvers::packages::mutation::PackagesMutation;
use crate::graphql::resolvers::sms::mutation::SmsMutation;
use plain_graphql::MergedObject;

#[derive(MergedObject, Default)]
pub struct MobileMutation(
    PackagesMutation,
    ContactsMutation,
    SmsMutation,
    CallsMutation,
);
