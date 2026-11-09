use plain_rs::features::prefs::Prefs;
use plain_rs::infrastructure::db::Db;
use std::sync::Arc;

pub(crate) fn runtime(
    host: Arc<plain_rs::infrastructure::host::bridge::Host>,
    events: tokio::sync::broadcast::Sender<plain_rs::infrastructure::events::WsEvent>,
    prefs: Arc<Prefs>,
    db: Arc<Db>,
    directory: std::path::PathBuf,
) -> (plain_rs::runtime::Runtime, tokio::sync::watch::Sender<bool>) {
    struct Clients;
    impl plain_rs::infrastructure::host::client_connections::ClientConnections for Clients {
        fn close(&self, _: Option<&str>) {}
        fn revoke(&self, _: &str) -> Result<(), String> {
            Ok(())
        }
        fn contains(&self, _: &str) -> bool {
            false
        }
    }
    let data = plain_rs::infrastructure::data_paths::directory::DataDirectory::open(
        &directory,
        &directory.join("cache"),
    )
    .unwrap();
    let (runtime, stop) = plain_rs::runtime::Runtime::from_storage(
        &directory,
        &plain_libs::crypto::gen_token(),
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
    host: Arc<plain_rs::infrastructure::host::bridge::Host>,
    events: tokio::sync::broadcast::Sender<plain_rs::infrastructure::events::WsEvent>,
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

pub(crate) struct SchemaFixture {
    schema: crate::graphql::schemas::public::PublicSchema,
    _stop: tokio::sync::watch::Sender<bool>,
}
impl std::ops::Deref for SchemaFixture {
    type Target = crate::graphql::schemas::public::PublicSchema;
    fn deref(&self) -> &Self::Target {
        &self.schema
    }
}
