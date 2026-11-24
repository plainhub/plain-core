use super::DlnaSenderState;
use crate::graphql::resolvers::app::capability_types::DlnaRenderer;
use crate::server::ServerState;
use plain_graphql::{Context, Object, Result};

#[derive(Default)]
pub struct DlnaSenderQuery;

#[Object]
impl DlnaSenderQuery {
    fn dlna_sender_state(&self, ctx: &Context<'_>) -> Result<DlnaSenderState> {
        ctx.data::<ServerState>()?.cast.snapshot().try_into()
    }

    fn dlna_renderers(&self, ctx: &Context<'_>) -> Result<Vec<DlnaRenderer>> {
        let state = ctx.data::<ServerState>()?;
        let devices = state
            .cast
            .devices
            .lock()
            .map_err(|_| plain_graphql::Error::new("DLNA device state unavailable"))?;
        let mut rows = devices
            .values()
            .filter_map(|device| device.to_renderer())
            .map(|row| DlnaRenderer {
                udn: row.udn,
                name: row.name,
                manufacturer: (!row.manufacturer.is_empty()).then_some(row.manufacturer),
                model_name: (!row.model_name.is_empty()).then_some(row.model_name),
                location: row.location,
            })
            .collect::<Vec<_>>();
        rows.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.udn.cmp(&b.udn)));
        Ok(rows)
    }
}
