pub mod mutation;
pub mod query;

use plain_graphql::{Context, Result};

fn caller<'a>(ctx: &'a Context<'_>) -> Result<&'a str> {
    ctx.data::<String>().map(String::as_str).and_then(|id| {
        if id.is_empty() {
            Err("authenticated caller required".into())
        } else {
            Ok(id)
        }
    })
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-server/unit/graphql/resolvers/device/sessions/mod.rs"]
mod tests;
