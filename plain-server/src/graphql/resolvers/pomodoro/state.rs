use async_graphql::Context;
use plain_graphql_macros::object as Object;
use plain_rs::features::services::pomodoro::PomodoroToday;
use plain_rs::features::services::pomodoro::Service;
use std::sync::Arc;
#[derive(Default)]
pub struct PomodoroControlMutation;
#[Object]
impl PomodoroControlMutation {
    fn adjust_pomodoro(
        &self,
        ctx: &Context<'_>,
        duration_sec: i32,
    ) -> async_graphql::Result<PomodoroToday> {
        Ok(ctx
            .data::<Arc<Service>>()?
            .command("adjust", Some(duration_sec))?)
    }
}
