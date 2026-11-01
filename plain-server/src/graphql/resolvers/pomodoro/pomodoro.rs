use async_graphql::{Context, Object};
pub use plain_rs::features::services::pomodoro::PomodoroSettings;
pub use plain_rs::features::services::pomodoro::PomodoroToday;
use plain_rs::features::services::pomodoro::Service;
use std::sync::Arc;
#[derive(Default)]
pub struct PomodoroQuery;
#[Object]
impl PomodoroQuery {
    fn pomodoro_settings(&self, ctx: &Context<'_>) -> async_graphql::Result<PomodoroSettings> {
        Ok(ctx.data::<Arc<Service>>()?.settings()?)
    }
    fn pomodoro_today(&self, ctx: &Context<'_>) -> async_graphql::Result<PomodoroToday> {
        Ok(ctx.data::<Arc<Service>>()?.today()?)
    }
}
#[derive(Default)]
pub struct PomodoroMutation;
#[Object]
impl PomodoroMutation {
    fn start_pomodoro(&self, ctx: &Context<'_>, duration_sec: i32) -> async_graphql::Result<bool> {
        ctx.data::<Arc<Service>>()?
            .command("start", Some(duration_sec))?;
        Ok(true)
    }
    fn pause_pomodoro(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
        ctx.data::<Arc<Service>>()?.command("pause", None)?;
        Ok(true)
    }
    fn stop_pomodoro(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
        ctx.data::<Arc<Service>>()?.command("stop", None)?;
        Ok(true)
    }
}
