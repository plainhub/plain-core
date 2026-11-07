// pest. The Elegant Parser
// Copyright (c) 2018 Dragoș Tiselice
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

#[macro_export]
macro_rules! consumes_to {
    ( $_rules:ident, $tokens:expr, [] ) => ();
    ( $rules:ident, $tokens:expr, [ $name:ident ( $start:expr, $end:expr ) ] ) => {
        let expected = format!("expected Start {{ rule: {:?}, pos: Position {{ pos: {} }} }}",
                               $rules::$name, $start);
        match $tokens.next().expect(&format!("{} but found nothing", expected)) {
            $crate::peg::Token::Start { rule, pos } => {
                assert!(
                    rule == $rules::$name && pos.pos() == $start,
                    "{} but found Start {{ rule: {:?}, pos: Position {{ {} }} }}",
                    expected, rule, pos.pos(),
                )
            },
            token => panic!("{} but found {:?}", expected, token)
        };

        let expected = format!("expected End {{ rule: {:?}, pos: Position {{ pos: {} }} }}",
                               $rules::$name, $end);
        match $tokens.next().expect(&format!("{} but found nothing", expected)) {
            $crate::peg::Token::End { rule, pos } => {
                assert!(rule == $rules::$name && pos.pos() == $end,
                    "{} but found End {{ rule: {:?}, pos: Position {{ {} }} }}",
                    expected, rule, pos.pos(),
                );
            },
            token => panic!("{} but found {:?}", expected, token)
        };
    };
    ( $rules:ident, $tokens:expr, [ $name:ident ( $start:expr, $end:expr ),
                                    $( $names:ident $calls:tt ),* $(,)* ] ) => {

        let expected = format!("expected Start {{ rule: {:?}, pos: Position {{ pos: {} }} }}",
                               $rules::$name, $start);
        match $tokens.next().expect(&format!("{} but found nothing", expected)) {
            $crate::peg::Token::Start { rule, pos } => {
                assert!(rule == $rules::$name && pos.pos() == $start,
                    "{} but found Start {{ rule: {:?}, pos: Position {{ {} }} }}",
                    expected, rule, pos.pos(),
                );
            },
            token => panic!("{} but found {:?}", expected, token)
        };

        let expected = format!("expected End {{ rule: {:?}, pos: Position {{ pos: {} }} }}",
                               $rules::$name, $end);
        match $tokens.next().expect(&format!("{} but found nothing", expected)) {
            $crate::peg::Token::End { rule, pos } => {
                assert!(rule == $rules::$name && pos.pos() == $end,
                    "{} but found End {{ rule: {:?}, pos: Position {{ {} }} }}",
                    expected, rule, pos.pos(),
                );
            },
            token => panic!("{} but found {:?}", expected, token)
        };

        consumes_to!($rules, $tokens, [ $( $names $calls ),* ]);
    };
    ( $rules:ident, $tokens:expr, [ $name:ident ( $start:expr, $end:expr,
                                                  [ $( $names:ident $calls:tt ),* $(,)* ] ) ] ) => {
        let expected = format!("expected Start {{ rule: {:?}, pos: Position {{ pos: {} }} }}",
                               $rules::$name, $start);
        match $tokens.next().expect(&format!("{} but found nothing", expected)) {
            $crate::peg::Token::Start { rule, pos } => {
                assert!(rule == $rules::$name && pos.pos() == $start,
                    "{} but found Start {{ rule: {:?}, pos: Position {{ {} }} }}",
                    expected, rule, pos.pos(),
                );
            },
            token => panic!("{} but found {:?}", expected, token)
        };

        consumes_to!($rules, $tokens, [ $( $names $calls ),* ]);

        let expected = format!("expected End {{ rule: {:?}, pos: Position {{ pos: {} }} }}",
                               $rules::$name, $end);
        match $tokens.next().expect(&format!("{} but found nothing", expected)) {
            $crate::peg::Token::End { rule, pos } => {
                assert!(rule == $rules::$name && pos.pos() == $end,
                    "{} but found End {{ rule: {:?}, pos: Position {{ {} }} }}",
                    expected, rule, pos.pos(),
                );
            },
            token => panic!("{} but found {:?}", expected, token)
        };
    };
    ( $rules:ident, $tokens:expr, [ $name:ident ( $start:expr, $end:expr,
                                                  [ $( $nested_names:ident $nested_calls:tt ),*
                                                  $(,)* ] ),
                                    $( $names:ident $calls:tt ),* ] ) => {

        let expected = format!("expected Start {{ rule: {:?}, pos: Position {{ pos: {} }} }}",
                               $rules::$name, $start);
        match $tokens.next().expect(&format!("{} but found nothing", expected)) {
            $crate::peg::Token::Start { rule, pos } => {
                assert!(rule == $rules::$name && pos.pos() == $start,
                    "{} but found Start {{ rule: {:?}, pos: Position {{ {} }} }}",
                    expected, rule, pos.pos(),
                );
            },
            token => panic!("{} but found {:?}", expected, token)
        };

        consumes_to!($rules, $tokens, [ $( $nested_names $nested_calls ),* ]);

        let expected = format!("expected End {{ rule: {:?}, pos: Position {{ pos: {} }} }}",
                               $rules::$name, $end);
        match $tokens.next().expect(&format!("{} but found nothing", expected)) {
            $crate::peg::Token::End { rule, pos } => {
                assert!(rule == $rules::$name && pos.pos() == $end,
                    "{} but found End {{ rule: {:?}, pos: Position {{ {} }} }}",
                    expected, rule, pos.pos(),
                );
            },
            token => panic!("{} but found {:?}", expected, token)
        };

        consumes_to!($rules, $tokens, [ $( $names $calls ),* ]);
    };
}

#[macro_export]
macro_rules! parses_to {
    ( parser: $parser:ident, input: $string:expr, rule: $rules:tt :: $rule:tt,
      tokens: [ $( $names:ident $calls:tt ),* $(,)* ] ) => {
        {

            use $crate::peg::Parser;

            let mut tokens = $parser::parse($rules::$rule, $string).unwrap().tokens();

            consumes_to!($rules, &mut tokens, [ $( $names $calls ),* ]);

            let rest: Vec<_> = tokens.collect();

            match rest.len() {
                0 => (),
                2 => {
                    let (first, second) = (&rest[0], &rest[1]);

                    match (first, second) {
                        (
                            &$crate::peg::Token::Start { rule: ref first_rule, .. },
                            &$crate::peg::Token::End { rule: ref second_rule, .. }
                        ) => {
                            assert!(
                                format!("{:?}", first_rule) == "EOI",
                                "expected end of input, but found {:?}", rest
                            );
                            assert!(
                                format!("{:?}", second_rule) == "EOI",
                                "expected end of input, but found {:?}", rest
                            );
                        }
                        _ => panic!("expected end of input, but found {:?}", rest)
                    }
                }
                _ => panic!("expected end of input, but found {:?}", rest)
            };
        }
    };
}

#[macro_export]
macro_rules! fails_with {
    ( parser: $parser:ident, input: $string:expr, rule: $rules:tt :: $rule:tt,
      positives: $positives:expr, negatives: $negatives:expr, pos: $pos:expr ) => {{
        use $crate::peg::Parser;

        let error = $parser::parse($rules::$rule, $string).unwrap_err();

        match error.variant {
            $crate::peg::error::ErrorVariant::ParsingError {
                positives,
                negatives,
            } => {
                assert_eq!(positives, $positives, "positives");
                assert_eq!(negatives, $negatives, "negatives");
            }
            _ => unreachable!(),
        };

        match error.location {
            $crate::peg::error::InputLocation::Pos(pos) => assert_eq!(pos, $pos, "pos"),
            _ => unreachable!(),
        }
    }};
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/peg/macros/tests.rs"]
pub(super) mod tests;
