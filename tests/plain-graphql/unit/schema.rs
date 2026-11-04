use super::*;
use crate::{Object, SimpleObject, value};
use futures_util::StreamExt;
use std::sync::atomic::{AtomicUsize, Ordering};

struct LayerValue(&'static str);
struct LayerQuery;
#[Object(internal)]
impl LayerQuery {
    fn layer(&self, ctx: &crate::Context<'_>) -> crate::Result<&'static str> {
        Ok(ctx.data::<LayerValue>()?.0)
    }
}

#[tokio::test]
async fn request_session_and_schema_data_keep_precedence_without_leaking_between_executions() {
    let schema = Schema::build(LayerQuery, EmptyMutation, EmptySubscription)
        .data(LayerValue("schema"))
        .finish();
    let mut session = Data::default();
    session.insert(LayerValue("session"));
    let session = Arc::new(session);
    let responses = schema
        .execute_stream_with_session_data(
            Request::new("{ layer }").data(LayerValue("query")),
            session.clone(),
        )
        .collect::<Vec<_>>()
        .await;
    assert_eq!(responses.len(), 1);
    assert!(responses[0].errors.is_empty());
    assert_eq!(responses[0].data, value!({"layer":"query"}));
    let responses = schema
        .execute_stream_with_session_data("{ layer }", session)
        .collect::<Vec<_>>()
        .await;
    assert_eq!(responses.len(), 1);
    assert!(responses[0].errors.is_empty());
    assert_eq!(responses[0].data, value!({"layer":"session"}));
    let response = schema.execute("{ layer }").await;
    assert!(response.errors.is_empty());
    assert_eq!(response.data, value!({"layer":"schema"}));
    let missing = Schema::new(LayerQuery, EmptyMutation, EmptySubscription)
        .execute("{ layer }")
        .await;
    assert_eq!(missing.data, crate::Value::Null);
    assert_eq!(missing.errors.len(), 1);
    assert!(missing.errors[0].message.contains("LayerValue"));
}

struct NamedMutation(Arc<AtomicUsize>);
#[Object(internal)]
impl NamedMutation {
    fn step(&self, delta: usize) -> usize {
        self.0.fetch_add(delta, Ordering::SeqCst) + delta
    }
}
#[tokio::test]
async fn operation_selection_remains_exact_without_storing_the_selected_name() {
    let calls = Arc::new(AtomicUsize::new(0));
    let schema = Schema::new(LayerQuery, NamedMutation(calls.clone()), EmptySubscription);
    let document = "mutation First { step(delta: 1) } mutation Second { step(delta: 10) }";
    let response = schema
        .execute(Request::new(document).operation_name("Second"))
        .await;
    assert!(response.errors.is_empty());
    assert_eq!(response.data, value!({"step":10}));
    for request in [
        Request::new(document).operation_name("Missing"),
        Request::new(document),
    ] {
        let response = schema.execute(request).await;
        assert!(!response.errors.is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 10);
    }
    for (document, expected) in [
        ("mutation Only { step(delta: 2) }", 12),
        ("mutation { step(delta: 3) }", 15),
    ] {
        let response = schema.execute(document).await;
        assert!(response.errors.is_empty());
        assert_eq!(response.data, value!({"step":expected}));
        assert_eq!(calls.load(Ordering::SeqCst), expected);
    }
}

#[derive(SimpleObject)]
#[graphql(internal, name = "ConflictingType")]
struct FirstType {
    value: i32,
}
#[derive(SimpleObject)]
#[graphql(internal, name = "ConflictingType")]
struct SecondType {
    value: bool,
}
struct ConflictingQuery;
#[Object(internal)]
impl ConflictingQuery {
    fn first(&self) -> FirstType {
        FirstType { value: 1 }
    }
    fn second(&self) -> SecondType {
        SecondType { value: false }
    }
}
#[test]
fn conflicting_type_names_still_fail_during_schema_building() {
    assert!(
        std::panic::catch_unwind(|| Schema::new(
            ConflictingQuery,
            EmptyMutation,
            EmptySubscription
        ))
        .is_err()
    );
}

struct AccessState {
    allowed: std::sync::atomic::AtomicBool,
    checks: AtomicUsize,
    reads: AtomicUsize,
}
struct AccessGuard;
impl crate::Guard for AccessGuard {
    async fn check(&self, ctx: &crate::Context<'_>) -> crate::Result<()> {
        let state = ctx.data::<Arc<AccessState>>()?;
        state.checks.fetch_add(1, Ordering::SeqCst);
        if state.allowed.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(crate::Error::new("denied"))
        }
    }
}
struct GuardQuery;
#[Object(internal)]
impl GuardQuery {
    #[graphql(guard = "AccessGuard")]
    fn guarded(&self, ctx: &crate::Context<'_>) -> i32 {
        ctx.data_unchecked::<Arc<AccessState>>()
            .reads
            .fetch_add(1, Ordering::SeqCst);
        7
    }
}
#[tokio::test]
async fn guards_run_once_before_resolvers_and_observe_current_permissions() {
    let state = Arc::new(AccessState {
        allowed: false.into(),
        checks: 0.into(),
        reads: 0.into(),
    });
    let schema = Schema::build(GuardQuery, EmptyMutation, EmptySubscription)
        .data(state.clone())
        .finish();
    let denied = schema.execute("{ alias: guarded }").await;
    assert_eq!(denied.errors.len(), 1);
    assert_eq!(denied.errors[0].message, "denied");
    assert_eq!(
        denied.errors[0].path,
        vec![crate::PathSegment::Field("alias".into())]
    );
    assert_eq!(state.checks.load(Ordering::SeqCst), 1);
    assert_eq!(state.reads.load(Ordering::SeqCst), 0);
    state.allowed.store(true, Ordering::SeqCst);
    let allowed = schema.execute("{ alias: guarded }").await;
    assert!(allowed.errors.is_empty());
    assert_eq!(allowed.data, value!({"alias":7}));
    assert_eq!(state.checks.load(Ordering::SeqCst), 2);
    assert_eq!(state.reads.load(Ordering::SeqCst), 1);
}
