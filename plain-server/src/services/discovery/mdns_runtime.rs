use crate::assembly::server::ServerState;
use crate::services::discovery::nearby_devices::Context;
use anyhow::{Result, ensure};
use plain_rs::protocols::mdns::host_responder;
use plain_rs::protocols::mdns::service_browser::MdnsServiceBrowser;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex, RwLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
const WS_MDNS_UPDATED: &'static str = "MDNS_UPDATED";
#[cfg(test)]
pub(crate) static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static OWNER: tokio::sync::Mutex<Option<String>> = tokio::sync::Mutex::const_new(None);
struct Session {
    browser: MdnsServiceBrowser,
    /// macOS only. The system DNS-SD browser sees devices the sandboxed
    /// browser cannot, which is what nearby discovery is for.
    #[cfg(target_os = "macos")]
    system_browser: crate::services::discovery::local::macos_dns_sd::MacDnsSdBrowser,
    hostname: Arc<RwLock<String>>,
    scanning: Arc<AtomicBool>,
    context: Context,
    published: bool,
    manual_scanning: bool,
    debug_sessions: std::collections::HashSet<uuid::Uuid>,
    discovery_sessions: plain_rs::discovery::sessions::DiscoverySessions,
}
impl Session {
    fn set_scanning(&self, active: bool) -> Result<bool> {
        if self.scanning.load(Ordering::SeqCst) == active {
            return Ok(false);
        }
        self.context.devices.scanning_modes(
            Some(active),
            None,
            if active {
                plain_rs::db::chat_store::nearby::all(&self.context.db)?
            } else {
                vec![]
            },
        )?;
        let previous = self.scanning.swap(active, Ordering::SeqCst);
        if previous == active {
            return Ok(false);
        }
        if active {
            self.browser.start();
        } else {
            self.browser.stop();
        }
        let _ = self
            .context
            .events
            .send(plain_rs::events::WsEvent::broadcast(
                if active {
                    "NEARBY_DISCOVERY_STARTED"
                } else {
                    "NEARBY_DISCOVERY_STOPPED"
                },
                "{}".into(),
            ));
        Ok(active)
    }
}
pub(crate) struct Runtime {
    id: String,
    session: Mutex<Option<Session>>,
    operation: tokio::sync::Mutex<()>,
    revision: AtomicU64,
    network_restart: Mutex<Option<tokio::task::JoinHandle<()>>>,
    #[cfg(test)]
    pub(crate) full_snapshot_count: AtomicU64,
}
impl Default for Runtime {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session: Mutex::new(None),
            operation: tokio::sync::Mutex::new(()),
            revision: AtomicU64::new(0),
            network_restart: Mutex::new(None),
            #[cfg(test)]
            full_snapshot_count: AtomicU64::new(0),
        }
    }
}
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum Request {
    Acquire { token: uuid::Uuid, owner: String },
    Release { token: uuid::Uuid, owner: String },
    Receiver {},
    Start {},
    Stop {},
    Restart {},
    Publish {},
    Update {},
    Unpublish {},
    Snapshot {},
    DebugStart { token: String },
    DebugStop { token: String },
}
impl Runtime {
    pub(crate) async fn update_hostname(self: &Arc<Self>, state: &ServerState) -> Result<()> {
        let hostname = state
            .prefs
            .get::<String>(plain_rs::prefs::pref_keys::MDNS_HOSTNAME)?
            .ok_or_else(|| anyhow::anyhow!("Missing hostname"))?;
        {
            let guard = self.session.lock().unwrap();
            let Some(session) = guard.as_ref() else {
                return Ok(());
            };
            let mut current = session.hostname.write().unwrap();
            if *current != hostname {
                *current = hostname;
                session.browser.clear_instances();
                session.browser.send_ptr_query();
            }
        }
        self.execute(state, Request::Update {}).await?;
        Ok(())
    }

    fn emit(&self, context: &Context, peer: Option<&str>, online: bool) {
        let revision = self.revision.fetch_add(1, Ordering::SeqCst) + 1;
        let _ = context.events.send(plain_rs::events::WsEvent::broadcast(
            WS_MDNS_UPDATED,
            json!({"runtimeId":self.id,"revision":revision,"peerId":peer,"online":online})
                .to_string(),
        ));
    }
    pub(crate) fn browse_resident(&self) {
        if let Some(session) = self.session.lock().unwrap().as_ref() {
            session.browser.send_ptr_query();
        }
    }
    pub(crate) fn browser_snapshot(
        &self,
    ) -> Vec<plain_rs::protocols::mdns::service_browser::MdnsServiceSnapshot> {
        self.session
            .lock()
            .unwrap()
            .as_ref()
            .map(browser_services)
            .unwrap_or_default()
    }
    pub(crate) fn browser_activity(&self) -> Vec<crate::services::discovery::local::MdnsActivity> {
        #[cfg(target_os = "macos")]
        {
            let guard = self.session.lock().unwrap();
            guard.as_ref().map(|session| session.system_browser.activity()).unwrap_or_default()
        }
        #[cfg(not(target_os = "macos"))]
        {
            Vec::new()
        }
    }

    pub(crate) fn snapshot(&self) -> Value {
        #[cfg(test)]
        self.full_snapshot_count.fetch_add(1, Ordering::Relaxed);
        let guard = self.session.lock().unwrap();
        let session = guard.as_ref();
        let (packets_in, packets_out) = if session.is_some() {
            plain_rs::protocols::mdns::packet_capture::snapshot()
        } else {
            (vec![], vec![])
        };
        let services = session.map(browser_services).unwrap_or_default();
        json!({"runtimeId":self.id,"packetsIn":packets_in,"packetsOut":packets_out,"revision":self.revision.load(Ordering::SeqCst),"receiver":session.is_some(),"running":session.is_some() && host_responder::is_running(),"scanning":session.is_some_and(|s|s.scanning.load(Ordering::SeqCst)),"published":session.is_some_and(|s|s.published),"manualScanning":session.is_some_and(|s|s.manual_scanning),"debugSessionCount":session.map(|s|s.debug_sessions.len()).unwrap_or(0),"capturing":session.is_some_and(|s|!s.debug_sessions.is_empty()),"services":services})
    }
    async fn initialize(self: &Arc<Self>, state: &ServerState) -> Result<()> {
        let mut owner = OWNER.lock().await;
        ensure!(
            owner.as_ref().is_none_or(|id| id == &self.id),
            "mDNS is owned by another runtime"
        );
        if self.session.lock().unwrap().is_some() {
            ensure!(
                state
                    .host
                    .call("mdnsMulticast", json!({"acquire":true,"lease":self.id}))
                    .await
                    .map_err(anyhow::Error::msg)?
                    .as_bool()
                    == Some(true),
                "Multicast permission unavailable"
            );
            return Ok(());
        }
        let id = state
            .prefs
            .get::<String>(plain_rs::prefs::pref_keys::CLIENT_ID)?
            .unwrap_or_default();
        ensure!(!id.is_empty(), "Missing client identity");
        let hostname = Arc::new(RwLock::new(
            state
                .prefs
                .get::<String>(plain_rs::prefs::pref_keys::MDNS_HOSTNAME)?
                .filter(|hostname| !hostname.is_empty())
                .ok_or_else(|| anyhow::anyhow!("Missing mDNS hostname"))?,
        ));
        let scanning = Arc::new(AtomicBool::new(false));
        let context = Context::from(state);
        let weak = Arc::downgrade(self);
        let observer = context.clone();
        let status = state.peer_status.clone();
        let status_db = state.db.clone();
        let status_events = state.events.clone();
        let visible = scanning.clone();
        let browser = MdnsServiceBrowser::new(id, hostname.clone(), move |found| {
            let Some(runtime) = weak.upgrade() else {
                return;
            };
            let guard = runtime.session.lock().unwrap();
            if guard.is_none() {
                return;
            }
            let id = found.id.clone();
            let device = plain_rs::discovery::nearby_devices::Device {
                id: found.id,
                name: found.name,
                ips: found.ips.into_iter().chain(found.ipv6).collect(),
                port: found.port,
                device_type: serde_json::from_value(json!(found.device_type))
                    .unwrap_or(plain_rs::chat::enums::DeviceType::Other)
                    .as_str()
                    .into(),
                version: found.version,
                platform: found.platform,
                last_seen_at: plain_rs::db::now_iso(),
                discovery_methods: vec!["LAN".into()],
            };
            match observer.seen(device, visible.load(Ordering::SeqCst), true) {
                Ok(true) => {
                    if visible.load(Ordering::SeqCst)
                        && status.connections.hint(&status_db, &id).unwrap_or(false)
                    {
                        let _ = status_events.send(plain_rs::events::WsEvent::broadcast(
                            plain_rs::chat::events::WS_PEER_STATUS_UPDATED,
                            json!({"id":id,"online":true}).to_string(),
                        ));
                        let _=status_events.send(plain_rs::events::WsEvent::broadcast("PEER_CONNECTIONS_UPDATED",json!({"runtimeId":status.connections.snapshot(&status_db)["runtimeId"]}).to_string()));
                    }
                    runtime.emit(&observer, Some(&id), visible.load(Ordering::SeqCst));
                }
                Ok(false) => {}
                Err(error) => log::warn!("mDNS observation: {error}"),
            }
        });
        let addrs = plain_rs::db::chat_store::peers::all(&state.db)?
            .into_iter()
            .flat_map(|p| p.ip.split(',').map(str::to_owned).collect::<Vec<_>>())
            .take(512)
            .collect::<Vec<_>>();
        *owner = Some(self.id.clone());
        ensure!(
            state
                .host
                .call("mdnsMulticast", json!({"acquire":true,"lease":self.id}))
                .await
                .map_err(anyhow::Error::msg)?
                .as_bool()
                == Some(true),
            "Multicast permission unavailable"
        );
        browser.seed_known_addrs(&addrs);
        browser.install_listener();
        // The system browser reports the same device shape as the sandboxed
        // one, so it feeds the same observer and the same event stream.
        #[cfg(target_os = "macos")]
        let system_browser = {
            let system_observer = context.clone();
            let system_visible = scanning.clone();
            let system_weak = Arc::downgrade(self);
            let system_events = state.events.clone();
            let system_status = state.peer_status.clone();
            let system_db = state.db.clone();
            let system_browser =
                crate::services::discovery::local::macos_dns_sd::MacDnsSdBrowser::new(
                    move |found| {
                        let Some(runtime) = system_weak.upgrade() else {
                            return;
                        };
                        if runtime.session.lock().unwrap().is_none() {
                            return;
                        }
                        let id = found.id.clone();
                        let device = plain_rs::discovery::nearby_devices::Device {
                            id: found.id,
                            name: found.name,
                            ips: found.ips,
                            port: found.port,
                            device_type: serde_json::from_value(json!(found.device_type))
                                .unwrap_or(plain_rs::chat::enums::DeviceType::Other)
                                .as_str()
                                .into(),
                            version: found.version,
                            platform: found.platform,
                            last_seen_at: plain_rs::db::now_iso(),
                            discovery_methods: vec!["LAN".into()],
                        };
                        match system_observer.seen(
                            device,
                            system_visible.load(Ordering::SeqCst),
                            true,
                        ) {
                            Ok(true) => {
                                if system_visible.load(Ordering::SeqCst)
                                    && system_status
                                        .connections
                                        .hint(&system_db, &id)
                                        .unwrap_or(false)
                                {
                                    let _ =
                                        system_events.send(plain_rs::events::WsEvent::broadcast(
                                            plain_rs::chat::events::WS_PEER_STATUS_UPDATED,
                                            json!({"id":id,"online":true}).to_string(),
                                        ));
                                }
                                runtime.emit(
                                    &system_observer,
                                    Some(&id),
                                    system_visible.load(Ordering::SeqCst),
                                );
                            }
                            Ok(false) => {}
                            Err(error) => log::warn!("system DNS-SD observation: {error}"),
                        }
                    },
                );
            system_browser.start();
            system_browser
        };
        *self.session.lock().unwrap() = Some(Session {
            browser,
            #[cfg(target_os = "macos")]
            system_browser,
            hostname: hostname.clone(),
            scanning,
            context,
            published: false,
            manual_scanning: false,
            debug_sessions: std::collections::HashSet::new(),
            discovery_sessions: Default::default(),
        });
        *owner = Some(self.id.clone());
        host_responder::ensure_started(&hostname.read().unwrap());
        Ok(())
    }
    pub(crate) async fn execute(
        self: &Arc<Self>,
        state: &ServerState,
        request: Request,
    ) -> Result<Value> {
        if matches!(request, Request::Snapshot {}) {
            return Ok(self.snapshot());
        }
        let _operation = self.operation.lock().await;
        if matches!(
            request,
            Request::Stop {}
                | Request::Release { .. }
                | Request::Unpublish {}
                | Request::Restart {}
                | Request::Update {}
                | Request::DebugStop { .. }
        ) && self.session.lock().unwrap().is_none()
        {
            return Ok(self.snapshot());
        }
        if matches!(request, Request::Update {})
            && !self
                .session
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|session| session.published)
        {
            return Ok(self.snapshot());
        }
        if !matches!(
            request,
            Request::Stop {}
                | Request::Release { .. }
                | Request::Unpublish {}
                | Request::DebugStop { .. }
        ) {
            crate::services::capabilities::require(&state.host, "PEER_DISCOVERY").await?;
        }
        if !matches!(
            request,
            Request::Stop {}
                | Request::Release { .. }
                | Request::Unpublish {}
                | Request::DebugStop { .. }
        ) {
            self.initialize(state).await?;
        }
        let service = if matches!(request, Request::Publish {} | Request::Update {}) {
            Some(serde_json::from_value::<
                plain_rs::protocols::mdns::service_info::MdnsServiceInfo,
            >(
                crate::services::discovery::discovery_advertisement::execute(
                    state,
                    crate::services::discovery::discovery_advertisement::Request::Mdns {},
                )
                .await?,
            )?)
        } else {
            None
        };
        let mut started = false;
        {
            let mut guard = self.session.lock().unwrap();
            let session = guard.as_mut().unwrap();
            match request {
                Request::Acquire { token, owner } => {
                    let mut sessions = session.discovery_sessions.clone();
                    sessions.acquire(&owner, token)?;
                    started = session.set_scanning(true)?;
                    session.discovery_sessions = sessions;
                }
                Request::Release { token, owner } => {
                    let mut sessions = session.discovery_sessions.clone();
                    sessions.release(&owner, token)?;
                    session.set_scanning(
                        session.manual_scanning
                            || !session.debug_sessions.is_empty()
                            || sessions.active(),
                    )?;
                    session.discovery_sessions = sessions;
                }
                Request::Receiver {} => {}
                Request::DebugStart { token } => {
                    let token = uuid::Uuid::parse_str(&token)?;
                    ensure!(
                        session.debug_sessions.contains(&token)
                            || session.debug_sessions.len() < 16,
                        "Too many mDNS debug sessions"
                    );
                    started = session.set_scanning(true)?;
                    session.debug_sessions.insert(token);
                    plain_rs::protocols::mdns::packet_capture::set_enabled(true);
                }
                Request::DebugStop { token } => {
                    let token = uuid::Uuid::parse_str(&token)?;
                    session.set_scanning(
                        session.manual_scanning
                            || session.discovery_sessions.active()
                            || session
                                .debug_sessions
                                .iter()
                                .any(|existing| existing != &token),
                    )?;
                    session.debug_sessions.remove(&token);
                    plain_rs::protocols::mdns::packet_capture::set_enabled(
                        !session.debug_sessions.is_empty(),
                    );
                }
                Request::Start {} => {
                    started = session.set_scanning(true)?;
                    session.manual_scanning = true;
                }
                Request::Stop {} => {
                    session.set_scanning(
                        !session.debug_sessions.is_empty() || session.discovery_sessions.active(),
                    )?;
                    session.manual_scanning = false;
                }
                Request::Restart {} => {
                    host_responder::restart_socket();
                    host_responder::ensure_ipv6_started();
                }
                Request::Publish {} => {
                    let service = service.unwrap();
                    *session.hostname.write().unwrap() = service.target_hostname.clone();
                    host_responder::clear_service();
                    host_responder::start(&service.target_hostname, Some(service.clone()));
                    session.published = true;
                }
                Request::Update {} => {
                    let service = service.unwrap();
                    if session.published {
                        *session.hostname.write().unwrap() = service.target_hostname.clone();
                        host_responder::update_service(service);
                    }
                }
                Request::Unpublish {} => {
                    host_responder::clear_service();
                    session.published = false;
                }
                Request::Snapshot {} => unreachable!(),
            }
            self.emit(&session.context, None, false);
        }
        let mut snapshot = self.snapshot();
        snapshot["started"] = json!(started);
        Ok(snapshot)
    }
    fn release_discovery_owner_locked(&self, owner: &str) -> Result<()> {
        let mut guard = self.session.lock().unwrap();
        let Some(session) = guard.as_mut() else {
            return Ok(());
        };
        let mut sessions = session.discovery_sessions.clone();
        if !sessions.release_owner(owner) {
            return Ok(());
        }
        session.set_scanning(
            session.manual_scanning || !session.debug_sessions.is_empty() || sessions.active(),
        )?;
        session.discovery_sessions = sessions;
        self.emit(&session.context, None, false);
        Ok(())
    }

    pub(crate) async fn release_native_scope(
        &self,
        scope: &crate::native::graphql::NativeRequestScope,
    ) -> Result<()> {
        let _operation = self.operation.lock().await;
        self.release_discovery_owner_locked(&format!("native:{}", scope.id()))
    }

    pub(crate) async fn release_discovery_client(
        &self,
        state: &ServerState,
        client_id: &str,
    ) -> Result<()> {
        let _operation = self.operation.lock().await;
        if state.subscription_sessions.snapshot().iter().any(|client| client == client_id)
        {
            return Ok(());
        }
        self.release_discovery_owner_locked(&remote_discovery_owner(client_id))
    }

    pub(crate) async fn host_connected(state: ServerState, generation: u64) {
        if state.native_client_id().is_err() {
            return;
        }
        let mut stop = state.stop.clone();
        for _ in 0..8 {
            if *stop.borrow() || !state.host.is_current(generation) {
                return;
            }
            let result = tokio::select! {
                _ = stop.changed() => return,
                result = state.mdns.execute(&state, Request::Receiver {}) => result,
            };
            match result {
                Ok(_) => return,
                Err(error) if error.to_string().contains("capacity exceeded") => {
                    tokio::select! { _ = stop.changed() => return, _ = tokio::time::sleep(std::time::Duration::from_millis(250)) => {} }
                }
                Err(error) => {
                    log::warn!("mDNS receiver unavailable: {error}");
                    return;
                }
            }
        }
    }
    pub(crate) fn network_changed(self: &Arc<Self>, state: ServerState) {
        if state.native_client_id().is_err() {
            return;
        }
        let runtime = self.clone();
        let stop = state.stop.clone();
        self.schedule_network_restart(stop, async move {
            let request = if runtime.session.lock().unwrap().is_some() {
                Request::Restart {}
            } else {
                Request::Receiver {}
            };
            if let Err(error) = runtime.execute(&state, request).await {
                log::debug!("mDNS network refresh failed: {error}");
            }
        });
    }
    pub(crate) fn permissions_changed(self: &Arc<Self>, state: ServerState) {
        if self.session.lock().unwrap().is_some() {
            return;
        }
        self.network_changed(state);
    }
    fn schedule_network_restart<F>(
        &self,
        mut stop: tokio::sync::watch::Receiver<bool>,
        restart: F,
    ) -> bool
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        if *stop.borrow() {
            return false;
        }
        let mut pending = self.network_restart.lock().unwrap();
        if pending.as_ref().is_some_and(|task| !task.is_finished()) {
            return false;
        }
        *pending = Some(tokio::spawn(async move {
            tokio::select! {
                biased;
                _ = stop.changed() => {},
                _ = tokio::time::sleep(std::time::Duration::from_millis(800)) => {
                    tokio::select! {
                        biased;
                        _ = stop.changed() => {},
                        _ = restart => {},
                    }
                }
            }
        }));
        true
    }
    fn cancel_network_restart(&self) {
        if let Some(task) = self.network_restart.lock().unwrap().take() {
            task.abort();
        }
    }
    pub(crate) async fn close(&self, host: &crate::host::bridge::Host) {
        self.cancel_network_restart();
        let _operation = self.operation.lock().await;
        let mut owner = OWNER.lock().await;
        if let Some(session) = self.session.lock().unwrap().take() {
            session.scanning.store(false, Ordering::SeqCst);
            let _ = session
                .context
                .devices
                .scanning_modes(Some(false), None, vec![]);
            session.browser.shutdown();
        }
        if owner.as_ref() == Some(&self.id) {
            host_responder::stop();
            plain_rs::protocols::mdns::packet_capture::set_enabled(false);
            let _ = host
                .call("mdnsMulticast", json!({"acquire":false,"lease":self.id}))
                .await;
            *owner = None;
        }
    }
}
pub(crate) async fn control(state: &ServerState, request: Request) -> Result<Value> {
    let mut stop = state.stop.clone();
    ensure!(!*stop.borrow(), "Server stopped");
    tokio::select! {
        _ = stop.changed() => Err(anyhow::anyhow!("Server stopped")),
        result = state.mdns.execute(state, request) => result,
    }
}

#[cfg(all(test, feature = "http_transport"))]
#[path = "../../../tests/unit/services/discovery/mdns_runtime.rs"]
mod tests;

fn browser_services(
    session: &Session,
) -> Vec<plain_rs::protocols::mdns::service_browser::MdnsServiceSnapshot> {
    #[allow(unused_mut)]
    let mut services = session.browser.snapshot();
    #[cfg(target_os = "macos")]
    services.extend(session.system_browser.snapshot());
    services
}

pub(crate) fn remote_discovery_owner(client_id: &str) -> String {
    format!("remote:{client_id}")
}
