use super::NativeUploads;
use crate::server::ContentServer;
use plain_rs::features::prefs::Prefs;
use std::sync::Arc;

pub(super) fn fixture() -> (plain_libs::temporary::TempDir, ContentServer, NativeUploads) {
    let root = plain_libs::temporary::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&root.path().join("prefs.json")).unwrap());
    prefs.set("client_id", "local").unwrap();
    let server =
        ContentServer::start_core(root.path(), &plain_libs::crypto::gen_token(), prefs).unwrap();
    let uploads = server.native_uploads().unwrap();
    (root, server, uploads)
}
