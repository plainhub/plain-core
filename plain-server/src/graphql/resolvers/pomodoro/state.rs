use plain_graphql::{Context, Object};

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
    ) -> plain_graphql::Result<PomodoroToday> {
        Ok(ctx
            .data::<Arc<Service>>()?
            .command("adjust", Some(duration_sec))?)
    }
}
