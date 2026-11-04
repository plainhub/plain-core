use super::*;
use crate::{
    EmptyMutation, EmptySubscription, Object, Schema, SimpleObject, parser::parse_query,
    validation::visit,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(SimpleObject)]
#[graphql(internal)]
struct CostNode {
    a: i32,
    b: i32,
    next: Option<Box<CostNode>>,
}
fn node_value() -> CostNode {
    CostNode {
        a: 1,
        b: 2,
        next: None,
    }
}
struct CostQuery(Arc<AtomicUsize>);
#[Object(internal)]
impl CostQuery {
    fn value(&self) -> i32 {
        self.0.fetch_add(1, Ordering::SeqCst);
        1
    }
    fn node(&self) -> CostNode {
        self.0.fetch_add(1, Ordering::SeqCst);
        node_value()
    }
    fn nodes(&self) -> Vec<CostNode> {
        vec![node_value(), node_value()]
    }
}

#[test]
fn field_costs_include_aliases_nested_fields_lists_and_fragments() {
    let registry = Schema::<CostQuery, EmptyMutation, EmptySubscription>::create_registry();
    for (query, expected) in [
        ("{ value }", 1),
        ("{ first: value second: value }", 2),
        ("{ value node { a b } }", 4),
        ("{ nodes { a b } }", 3),
        ("{ node { next { a } b } }", 4),
        (
            "{ node { ...Fields } } fragment Fields on CostNode { a next { b } }",
            4,
        ),
        ("{ node { ... on CostNode { a b } } }", 3),
    ] {
        let doc = parse_query(query).unwrap();
        let mut ctx = VisitorContext::new(&registry, &doc, None, None);
        let mut complexity = 0;
        visit(
            &mut ComplexityCalculate::new(&mut complexity),
            &mut ctx,
            &doc,
        );
        assert!(ctx.errors.is_empty());
        assert_eq!(complexity, expected, "{query}");
    }
}

#[tokio::test]
async fn request_limits_reject_before_resolvers_and_accept_boundaries() {
    for (limit, query) in [
        ("complexity", "{ first: value second: value }"),
        ("depth", "{ node { a } }"),
        ("aliases", "{ alias: value }"),
        (
            "directives",
            "{ value @skip(if: false) @include(if: true) }",
        ),
        ("strict", "{ unknown }"),
    ] {
        let calls = Arc::new(AtomicUsize::new(0));
        let builder = Schema::build(CostQuery(calls.clone()), EmptyMutation, EmptySubscription);
        let schema = match limit {
            "complexity" => builder.limit_complexity(1),
            "depth" => builder.limit_depth(1),
            "aliases" => builder.limit_aliases(0),
            "directives" => builder.limit_directives(1),
            "strict" => builder,
            _ => unreachable!(),
        }
        .finish();
        let response = schema.execute(query).await;
        assert!(!response.errors.is_empty(), "{limit}: {response:?}");
        assert_eq!(calls.load(Ordering::SeqCst), 0, "{limit}");
        let response = schema.execute("{ value }").await;
        assert!(response.errors.is_empty(), "{limit}: {response:?}");
        assert_eq!(calls.load(Ordering::SeqCst), 1, "{limit}");
    }
}
