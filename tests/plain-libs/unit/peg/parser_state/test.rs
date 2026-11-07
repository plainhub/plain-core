use super::*;

type TestState = Box<ParserState<'static, ()>>;
type TestResult = ParseResult<TestState>;
type Combinator = fn(TestState, fn(TestState) -> TestResult) -> TestResult;

const COMBINATORS: [(&str, Combinator); 9] = [
    ("rule", |state, inner| state.rule((), inner)),
    ("sequence", ParserState::sequence),
    ("repeat", ParserState::repeat),
    ("optional", ParserState::optional),
    ("positive lookahead", |state, inner| {
        state.lookahead(true, inner)
    }),
    ("negative lookahead", |state, inner| {
        state.lookahead(false, inner)
    }),
    ("atomic", |state, inner| {
        state.atomic(Atomicity::Atomic, inner)
    }),
    ("stack_push", ParserState::stack_push),
    ("restore_on_err", ParserState::restore_on_err),
];

fn tracker_at_calls(calls: usize, limit: usize) -> CallLimitTracker {
    CallLimitTracker {
        current_call_limit: Some((calls, limit)),
        limit_reached: None,
    }
}

#[test]
fn stack_limit_probe_boundary_and_unknown_bounds() {
    for stack_space in [
        None,
        Some(REQUIRED_STACK_SPACE),
        Some(REQUIRED_STACK_SPACE - 1),
    ] {
        let mut tracker = tracker_at_calls(0, 10);
        let allowed = stack_space != Some(REQUIRED_STACK_SPACE - 1);
        assert_eq!(tracker.enter_with_stack(true, stack_space), allowed);
        assert_eq!(tracker.current_call_limit, Some((usize::from(allowed), 10)));
        if !allowed {
            assert_eq!(tracker.error_message(), Some("stack limit reached"));
            assert!(!tracker.enter_with_stack(true, None));
        }
    }
}

#[test]
fn call_limit_counts_shallow_work() {
    let result = state::<(), _>("xxxxxxxxxxxxxxxx", |mut state| {
        state.call_tracker = CallLimitTracker {
            current_call_limit: Some((0, 3)),
            limit_reached: None,
        };
        state.repeat(|state| state.sequence(|state| state.match_string("x")))
    });
    assert_eq!(
        result.unwrap_err().variant,
        ErrorVariant::CustomError {
            message: "call limit reached".to_owned()
        }
    );
}

#[test]
fn call_limit_preserves_combinator_charges() {
    for (name, combinator) in COMBINATORS {
        for input in ["x", "y"] {
            for initial_calls in [0, 2] {
                let mut state = ParserState::new(input);
                state.call_tracker = tracker_at_calls(initial_calls, 10);
                let result = combinator(state, |state| state.match_string("x"));
                let should_succeed = match name {
                    "repeat" | "optional" => true,
                    "negative lookahead" => input != "x",
                    _ => input == "x",
                };
                assert_eq!(result.is_ok(), should_succeed, "{name}");
                let state = result.unwrap_or_else(|state| state);
                assert_eq!(
                    state.call_tracker.current_call_limit,
                    Some((initial_calls + usize::from(name != "restore_on_err"), 10)),
                    "{name} on {input:?}"
                );
                assert!(!state.call_tracker.limit_reached(), "{name}");
                assert_eq!(state.lookahead, Lookahead::None, "{name}");
                assert_eq!(state.atomicity, Atomicity::NonAtomic, "{name}");
            }
        }
    }
}

#[test]
fn call_limit_boundary() {
    for (name, combinator) in COMBINATORS {
        if name == "restore_on_err" {
            continue;
        }
        for initial_calls in [0, 2] {
            for remaining in [1, 2] {
                let limit = initial_calls + remaining;
                let mut state = ParserState::new("x");
                state.call_tracker = tracker_at_calls(initial_calls, limit);
                let result = combinator(state, |state| {
                    state.rule((), |state| state.match_string("x"))
                });
                let state = result.unwrap_or_else(|state| state);
                assert_eq!(
                    state.call_tracker.current_call_limit,
                    Some((limit, limit)),
                    "{name}"
                );
                let exhausted = remaining == 1 || name == "repeat";
                assert_eq!(state.call_tracker.limit_reached(), exhausted, "{name}");
                if exhausted {
                    let state = combinator(state, |_| {
                        panic!("exhausted state must not reenter a helper")
                    })
                    .unwrap_err();
                    assert_eq!(
                        state.call_tracker.current_call_limit,
                        Some((limit, limit)),
                        "{name}"
                    );
                }
            }
        }
    }
}

#[test]
fn call_limit_restore_on_err_checkpoints() {
    for limit in [1, 2] {
        for succeeds in [false, true] {
            let mut state = ParserState::<()>::new("xy");
            state.call_tracker = tracker_at_calls(0, limit);
            state
                .stack
                .push(SpanOrLiteral::Span(Span::new("xy", 0, 1).unwrap()));
            let state = state.checkpoint();
            let result = state.restore_on_err(|mut state| {
                state.stack.pop();
                state.restore_on_err(|state| {
                    state.sequence(|mut state| {
                        state
                            .stack
                            .push(SpanOrLiteral::Span(Span::new("xy", 1, 2).unwrap()));
                        let state = state.match_string("x").unwrap();
                        if succeeds { Ok(state) } else { Err(state) }
                    })
                })
            });
            assert_eq!(result.is_ok(), succeeds);
            let state = result.unwrap_or_else(|state| state);
            assert_eq!(state.call_tracker.current_call_limit, Some((1, limit)));
            assert!(!state.call_tracker.limit_reached());
            assert_eq!(state.position.pos(), usize::from(succeeds));
            let top = state.stack.peek().unwrap().as_borrowed_or_rc();
            assert_eq!(top.as_str(), if succeeds { "y" } else { "x" });
            let state = state.restore();
            assert_eq!(state.stack.len(), 1);
            assert_eq!(
                state.stack.peek().unwrap().as_borrowed_or_rc().as_str(),
                "x"
            );
        }
    }
}

#[test]
fn call_limit_accumulates_on_backtracking() {
    let input = "xxxxxxxxxxxxxxxx";
    let mut state = ParserState::<()>::new(input);
    state.call_tracker = tracker_at_calls(0, 100);
    let state = state
        .repeat(|state| {
            state
                .sequence(|state| state.match_string("y"))
                .or_else(|state| state.sequence(|state| state.match_string("x")))
        })
        .unwrap();
    assert_eq!(state.position.pos(), input.len());
    assert_eq!(state.call_tracker.current_call_limit, Some((35, 100)));
    assert!(!state.call_tracker.limit_reached());
}

#[test]
fn call_limit_cannot_be_swallowed() {
    for (name, combinator) in COMBINATORS {
        for error_detail in [false, true] {
            let result = state::<(), _>("", |mut state| {
                state.call_tracker = tracker_at_calls(0, 1);
                state.parse_attempts.enabled = error_detail;
                combinator(state, |state| {
                    state.rule((), |state| {
                        state.rule((), |_| panic!("over-limit closure must not run"))
                    })
                })
                .or_else(Ok)
            });
            assert_eq!(
                result.expect_err(name).variant,
                ErrorVariant::CustomError {
                    message: "call limit reached".to_owned(),
                },
                "{name}, error_detail={error_detail}"
            );
        }
    }
}

#[test]
fn call_limit_stops_recovering_combinator_continuations() {
    for (name, combinator) in COMBINATORS {
        if !matches!(name, "optional" | "repeat" | "negative lookahead") {
            continue;
        }
        let mut state = ParserState::new("");
        state.call_tracker = tracker_at_calls(0, 1);
        let result: TestResult = combinator(state, |state| {
            state.rule((), |_| panic!("over-budget body must not run"))
        })
        .map(|_| panic!("fatal resource rejection must stop continuations"));
        assert!(result.is_err(), "{name}");
    }
}

#[test]
fn normalize_index_pos() {
    assert_eq!(normalize_index(4, 6), Some(4));
    assert_eq!(normalize_index(5, 5), Some(5));
    assert_eq!(normalize_index(6, 3), None);
}

#[test]
fn normalize_index_neg() {
    assert_eq!(normalize_index(-4, 6), Some(2));
    assert_eq!(normalize_index(-5, 5), Some(0));
    assert_eq!(normalize_index(-6, 3), None);
}
