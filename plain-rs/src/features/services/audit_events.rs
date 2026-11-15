mod writer;
use crate::infrastructure::db::AuditEventClient;
use crate::runtime::Runtime;
pub use writer::AuditEvents;

pub fn client(state: &Runtime, client_id: &str) -> plain_libs::anyhow::Result<AuditEventClient> {
    if state
        .native_client_id()
        .map_err(plain_libs::anyhow::Error::msg)?
        == client_id
    {
        let startup = state.host.startup();
        return Ok(AuditEventClient {
            client_id: client_id.to_owned(),
            client_name: crate::features::prefs::settings::snapshot(&state.prefs)?.device_name,
            os_name: startup.os_name.clone(),
            os_version: startup.os_version.clone(),
            app_name: "PlainApp".into(),
            app_version: startup.app_version.clone(),
            ..Default::default()
        });
    }
    Ok(state.db.audit_event_client(client_id)?)
}
