use plain_rs::{db::Db, prefs::Prefs};
use std::sync::Arc;

fn runtime(
    host: Arc<plain_rs::host::bridge::Host>,
    events: tokio::sync::broadcast::Sender<plain_rs::events::WsEvent>,
    prefs: Arc<Prefs>,
    db: Arc<Db>,
    directory: std::path::PathBuf,
) -> (plain_rs::runtime::Runtime, tokio::sync::watch::Sender<bool>) {
    struct Clients;
    impl plain_rs::host::client_connections::ClientConnections for Clients {
        fn close(&self, _: Option<&str>) {}
        fn revoke(&self, _: &str) -> Result<(), String> {
            Ok(())
        }
        fn contains(&self, _: &str) -> bool {
            false
        }
    }
    let data =
        plain_rs::data_paths::directory::DataDirectory::open(&directory, &directory.join("cache"))
            .unwrap();
    let (runtime, stop) = plain_rs::runtime::Runtime::from_storage(
        &directory,
        &plain_rs::crypto::gen_token(),
        prefs,
        host,
        Arc::new(Clients),
        db,
        data,
        events,
    )
    .unwrap();
    (runtime, stop)
}

pub(crate) fn build(
    host: Arc<plain_rs::host::bridge::Host>,
    events: tokio::sync::broadcast::Sender<plain_rs::events::WsEvent>,
    prefs: Arc<Prefs>,
    db: Arc<Db>,
    directory: std::path::PathBuf,
) -> SchemaFixture {
    let (runtime, stop) = runtime(host, events, prefs, db, directory);
    SchemaFixture {
        schema: crate::graphql::schemas::public::build_with_runtime(runtime),
        _stop: stop,
    }
}

pub(crate) type MobileSchema = async_graphql::Schema<
    crate::graphql::schemas::public::Query<crate::graphql::resolvers::mobile::MobileQuery>,
    crate::graphql::schemas::public::Mutation<crate::graphql::resolvers::mobile::MobileMutation>,
    crate::graphql::subscriptions::Subscription<
        crate::graphql::subscriptions::sms::SmsSubscriptions,
    >,
>;
pub(crate) type MobileFixture = SchemaFixture<MobileSchema>;
pub(crate) fn mobile(runtime: plain_rs::runtime::Runtime) -> MobileSchema {
    let builder =
        async_graphql::Schema::build(Default::default(), Default::default(), Default::default());
    crate::graphql::schemas::public::configure_schema(runtime, builder).finish()
}
pub(crate) fn build_mobile(
    host: Arc<plain_rs::host::bridge::Host>,
    events: tokio::sync::broadcast::Sender<plain_rs::events::WsEvent>,
    prefs: Arc<Prefs>,
    db: Arc<Db>,
    directory: std::path::PathBuf,
) -> MobileFixture {
    let (runtime, stop) = runtime(host, events, prefs, db, directory);
    SchemaFixture {
        schema: mobile(runtime),
        _stop: stop,
    }
}

pub(crate) struct SchemaFixture<T = crate::graphql::schemas::public::PublicSchema> {
    schema: T,
    _stop: tokio::sync::watch::Sender<bool>,
}
impl<T> std::ops::Deref for SchemaFixture<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.schema
    }
}
