use plain_graphql::{Context, Object};
use plain_rs::features::services::pomodoro::{PomodoroSettings, PomodoroToday, Service};
use std::sync::Arc;

#[derive(Default)]
pub struct PomodoroQuery;

#[Object]
impl PomodoroQuery {
    fn pomodoro_settings(&self, ctx: &Context<'_>) -> plain_graphql::Result<PomodoroSettings> {
        Ok(ctx.data::<Arc<Service>>()?.settings()?)
    }
    fn pomodoro_today(&self, ctx: &Context<'_>) -> plain_graphql::Result<PomodoroToday> {
        Ok(ctx.data::<Arc<Service>>()?.today()?)
    }
}
