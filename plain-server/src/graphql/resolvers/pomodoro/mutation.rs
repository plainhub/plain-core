use plain_graphql::{Context, Object};
use plain_rs::features::services::pomodoro::{PomodoroToday, Service};
use std::sync::Arc;

#[derive(Default)]
pub struct PomodoroMutation;

#[Object]
impl PomodoroMutation {
    fn start_pomodoro(
        &self,
        ctx: &Context<'_>,
        duration_sec: i32,
    ) -> plain_graphql::Result<PomodoroToday> {
        Ok(ctx
            .data::<Arc<Service>>()?
            .command("start", Some(duration_sec))?)
    }
    fn pause_pomodoro(&self, ctx: &Context<'_>) -> plain_graphql::Result<PomodoroToday> {
        Ok(ctx.data::<Arc<Service>>()?.command("pause", None)?)
    }
    fn stop_pomodoro(&self, ctx: &Context<'_>) -> plain_graphql::Result<PomodoroToday> {
        Ok(ctx.data::<Arc<Service>>()?.command("stop", None)?)
    }
}

#[derive(Default)]
pub struct PomodoroControlMutation;

#[Object]
impl PomodoroControlMutation {
    fn set_pomodoro_remaining_time(
        &self,
        ctx: &Context<'_>,
        remaining_sec: i32,
    ) -> plain_graphql::Result<PomodoroToday> {
        Ok(ctx
            .data::<Arc<Service>>()?
            .command("adjust", Some(remaining_sec))?)
    }
}
