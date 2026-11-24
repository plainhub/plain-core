use super::MdnsState;
use crate::server::ServerState;
use plain_graphql::{Context, Object};

#[derive(Default)]
pub struct MdnsQuery;

#[Object]
impl MdnsQuery {
    fn is_discovering(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        let facts = ctx.data::<ServerState>()?.mdns.snapshot();
        Ok(facts["scanning"].as_bool().unwrap_or(false))
    }
    fn mdns_state(&self, ctx: &Context<'_>) -> plain_graphql::Result<MdnsState> {
        let state = ctx.data::<ServerState>()?;
        if *state.stop.borrow() {
            return Err("Server stopped".into());
        }
        Ok(plain_libs::serde_json::from_value(
            state.mdns.snapshot_fields(
                ctx.look_ahead().field("services").exists(),
                ctx.look_ahead().field("packetsIn").exists()
                    || ctx.look_ahead().field("packetsOut").exists(),
            ),
        )?)
    }
}
