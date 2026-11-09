use crate::server::ContentServer;
use plain_rs::{
    features::{
        access::AccessFeature,
        chat::enums::{DeviceType, PeerStatus},
        prefs::{Prefs, pref_keys},
        services::sessions,
    },
    infrastructure::db::{
        DPeer,
        chat_store::{SaveMode, peers},
    },
};
use std::sync::Arc;

pub(super) struct Fixture {
    pub root: plain_libs::temporary::TempDir,
    pub local: ContentServer,
    pub remote: ContentServer,
    pub peer: DPeer,
}

impl Fixture {
    pub async fn new() -> Self {
        let root = plain_libs::temporary::tempdir().unwrap();
        let start = |name: &str| {
            let path = root.path().join(name);
            std::fs::create_dir_all(&path).unwrap();
            let prefs = Arc::new(Prefs::load(&path.join("prefs.json")).unwrap());
            prefs.set("client_id", name).unwrap();
            prefs
                .set(pref_keys::API_PERMISSIONS, [AccessFeature::Files])
                .unwrap();
            ContentServer::start_core(&path, &plain_libs::crypto::gen_token(), prefs).unwrap()
        };
        let local = start("local");
        let remote = start("remote");
        let (cert, key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
        let (_, port) = remote.start_public(0, 0, cert, key).await.unwrap();
        let remote_state = remote.runtime_state();
        let mut session = sessions::create(&remote_state, "local".into()).unwrap();
        session.client_id = "local".into();
        session.r#type = plain_rs::features::sessions::SessionType::Login;
        remote_state.db.session_save(&session).unwrap();
        let mut peer = DPeer::new(
            "remote",
            "Fixture",
            "127.0.0.1",
            Some(port),
            DeviceType::Phone,
        );
        peer.status = PeerStatus::Paired;
        peer.public_key = plain_libs::utils::base64::base64_encode(&[7; 32]);
        peer.token = session.token;
        peers::save(&local.runtime_state().db, &[peer.clone()], SaveMode::Insert).unwrap();
        Self {
            root,
            local,
            remote,
            peer,
        }
    }
    pub async fn stop(self) {
        self.local.stop_core().await;
        self.remote.stop_core().await;
    }
}
