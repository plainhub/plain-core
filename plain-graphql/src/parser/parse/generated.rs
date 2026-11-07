use super::GraphQLParser;
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Rule {
    Eoi,
    LineTerminator,
    ExecutableDocument,
    ExecutableDefinition,
    OperationDefinition,
    NamedOperationDefinition,
    VariableDefinitions,
    VariableDefinition,
    SelectionSet,
    Selection,
    Field,
    Alias,
    FragmentSpread,
    InlineFragment,
    FragmentDefinition,
    TypeCondition,
    OperationType,
    DefaultValue,
    Type,
    ConstValue,
    Value,
    Variable,
    Number,
    Float,
    Fractional,
    Exponent,
    Int,
    String,
    BlockStringContent,
    BlockStringCharacter,
    StringContent,
    StringCharacter,
    UnicodeScalarValueHex,
    Boolean,
    Null,
    EnumValue,
    ConstList,
    List,
    ConstObject,
    Object,
    ConstObjectField,
    ObjectField,
    Directives,
    Directive,
    Arguments,
    Argument,
    NameStart,
    Name,
}
impl ::plain_libs::peg::Parser<Rule> for GraphQLParser {
    fn parse<'i>(
        rule: Rule,
        input: &'i str,
    ) -> ::std::result::Result<::plain_libs::peg::iterators::Pairs<'i, Rule>, ::plain_libs::peg::error::Error<Rule>> {
        ::plain_libs::peg::state(input, |state| match rule {
            Rule::LineTerminator => rules::r#line_terminator(state),
            Rule::ExecutableDocument => rules::r#executable_document(state),
            Rule::ExecutableDefinition => rules::r#executable_definition(state),
            Rule::OperationDefinition => rules::r#operation_definition(state),
            Rule::NamedOperationDefinition => rules::r#named_operation_definition(state),
            Rule::VariableDefinitions => rules::r#variable_definitions(state),
            Rule::VariableDefinition => rules::r#variable_definition(state),
            Rule::SelectionSet => rules::r#selection_set(state),
            Rule::Selection => rules::r#selection(state),
            Rule::Field => rules::r#field(state),
            Rule::Alias => rules::r#alias(state),
            Rule::FragmentSpread => rules::r#fragment_spread(state),
            Rule::InlineFragment => rules::r#inline_fragment(state),
            Rule::FragmentDefinition => rules::r#fragment_definition(state),
            Rule::TypeCondition => rules::r#type_condition(state),
            Rule::OperationType => rules::r#operation_type(state),
            Rule::DefaultValue => rules::r#default_value(state),
            Rule::Type => rules::r#type_(state),
            Rule::ConstValue => rules::r#const_value(state),
            Rule::Value => rules::r#value(state),
            Rule::Variable => rules::r#variable(state),
            Rule::Number => rules::r#number(state),
            Rule::Float => rules::r#float(state),
            Rule::Fractional => rules::r#fractional(state),
            Rule::Exponent => rules::r#exponent(state),
            Rule::Int => rules::r#int(state),
            Rule::String => rules::r#string(state),
            Rule::BlockStringContent => rules::r#block_string_content(state),
            Rule::BlockStringCharacter => rules::r#block_string_character(state),
            Rule::StringContent => rules::r#string_content(state),
            Rule::StringCharacter => rules::r#string_character(state),
            Rule::UnicodeScalarValueHex => rules::r#unicode_scalar_value_hex(state),
            Rule::Boolean => rules::r#boolean(state),
            Rule::Null => rules::r#null(state),
            Rule::EnumValue => rules::r#enum_value(state),
            Rule::ConstList => rules::r#const_list(state),
            Rule::List => rules::r#list(state),
            Rule::ConstObject => rules::r#const_object(state),
            Rule::Object => rules::r#object(state),
            Rule::ConstObjectField => rules::r#const_object_field(state),
            Rule::ObjectField => rules::r#object_field(state),
            Rule::Directives => rules::r#directives(state),
            Rule::Directive => rules::r#directive(state),
            Rule::Arguments => rules::r#arguments(state),
            Rule::Argument => rules::r#argument(state),
            Rule::NameStart => rules::r#name_start(state),
            Rule::Name => rules::r#name(state),
            Rule::Eoi => rules::eoi(state),
        })
    }
}
mod rules {
    pub mod hidden {
        use super::super::Rule;
        #[inline]
        pub fn skip(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            if state.atomicity() == ::plain_libs::peg::Atomicity::NonAtomic {
                state.sequence(|state| {
                    state
                        .repeat(|state| super::visible::whitespace(state))
                        .and_then(|state| {
                            state.repeat(|state| {
                                state.sequence(|state| {
                                    super::visible::comment(state).and_then(|state| {
                                        state.repeat(|state| super::visible::whitespace(state))
                                    })
                                })
                            })
                        })
                })
            } else {
                Ok(state)
            }
        }
    }
    pub mod visible {
        use super::super::Rule;
        #[inline]
        pub fn whitespace(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.atomic(::plain_libs::peg::Atomicity::Atomic, |state| {
                state
                    .match_string(" ")
                    .or_else(|state| state.match_string(","))
                    .or_else(|state| state.match_string("\t"))
                    .or_else(|state| state.match_string("\u{feff}"))
                    .or_else(|state| self::r#line_terminator(state))
            })
        }
        #[inline]
        pub fn comment(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.atomic(::plain_libs::peg::Atomicity::Atomic, |state| {
                state.sequence(|state| {
                    state.match_string("#").and_then(|state| {
                        state.repeat(|state| {
                            state.sequence(|state| {
                                state
                                    .lookahead(false, |state| self::r#line_terminator(state))
                                    .and_then(|state| self::r#any(state))
                            })
                        })
                    })
                })
            })
        }
        #[inline]
        pub fn r#line_terminator(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::LineTerminator, |state| {
                state.atomic(::plain_libs::peg::Atomicity::Atomic, |state| {
                    state
                        .match_string("\r\n")
                        .or_else(|state| state.match_string("\r"))
                        .or_else(|state| state.match_string("\n"))
                })
            })
        }
        #[inline]
        pub fn r#executable_document(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::ExecutableDocument, |state| {
                state.sequence(|state| {
                    self::r#soi(state)
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| {
                            state.sequence(|state| {
                                self::r#executable_definition(state)
                                    .and_then(|state| super::hidden::skip(state))
                                    .and_then(|state| {
                                        state.sequence(|state| {
                                            state.optional(|state| {
                                                self::r#executable_definition(state).and_then(|state| {
                                                    state.repeat(|state| {
                                                        state.sequence(|state| {
                                                            super::hidden::skip(state).and_then(|state| {
                                                                self::r#executable_definition(state)
                                                            })
                                                        })
                                                    })
                                                })
                                            })
                                        })
                                    })
                            })
                        })
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| self::r#eoi(state))
                })
            })
        }
        #[inline]
        pub fn r#executable_definition(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::ExecutableDefinition, |state| {
                self::r#operation_definition(state).or_else(|state| self::r#fragment_definition(state))
            })
        }
        #[inline]
        pub fn r#operation_definition(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::OperationDefinition, |state| {
                self::r#named_operation_definition(state).or_else(|state| self::r#selection_set(state))
            })
        }
        #[inline]
        pub fn r#named_operation_definition(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::NamedOperationDefinition, |state| {
                state.sequence(|state| {
                    self::r#operation_type(state)
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.optional(|state| self::r#name(state)))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.optional(|state| self::r#variable_definitions(state)))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.optional(|state| self::r#directives(state)))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| self::r#selection_set(state))
                })
            })
        }
        #[inline]
        pub fn r#variable_definitions(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::VariableDefinitions, |state| {
                state.sequence(|state| {
                    state
                        .match_string("(")
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| {
                            state.sequence(|state| {
                                state.optional(|state| {
                                    self::r#variable_definition(state).and_then(|state| {
                                        state.repeat(|state| {
                                            state.sequence(|state| {
                                                super::hidden::skip(state)
                                                    .and_then(|state| self::r#variable_definition(state))
                                            })
                                        })
                                    })
                                })
                            })
                        })
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.match_string(")"))
                })
            })
        }
        #[inline]
        pub fn r#variable_definition(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::VariableDefinition, |state| {
                state.sequence(|state| {
                    self::r#variable(state)
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.match_string(":"))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| self::r#type_(state))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.optional(|state| self::r#directives(state)))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.optional(|state| self::r#default_value(state)))
                })
            })
        }
        # [inline] pub fn r#selection_set (state : :: std :: boxed :: Box < :: plain_libs :: peg :: ParserState < '_ , Rule > >) -> :: plain_libs :: peg :: ParseResult < :: std :: boxed :: Box < :: plain_libs :: peg :: ParserState < '_ , Rule > > > { state . rule (Rule :: SelectionSet , | state | { state . sequence (| state | { state . match_string ("{") . and_then (| state | { super :: hidden :: skip (state) }) . and_then (| state | { state . sequence (| state | { self :: r#selection (state) . and_then (| state | { super :: hidden :: skip (state) }) . and_then (| state | { state . sequence (| state | { state . optional (| state | { self :: r#selection (state) . and_then (| state | { state . repeat (| state | { state . sequence (| state | { super :: hidden :: skip (state) . and_then (| state | { self :: r#selection (state) }) }) }) }) }) }) }) }) }) . and_then (| state | { super :: hidden :: skip (state) }) . and_then (| state | { state . match_string ("}") }) }) }) }
        #[inline]
        pub fn r#selection(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::Selection, |state| {
                self::r#field(state)
                    .or_else(|state| self::r#inline_fragment(state))
                    .or_else(|state| self::r#fragment_spread(state))
            })
        }
        #[inline]
        pub fn r#field(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::Field, |state| {
                state.sequence(|state| {
                    state
                        .optional(|state| self::r#alias(state))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| self::r#name(state))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.optional(|state| self::r#arguments(state)))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.optional(|state| self::r#directives(state)))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.optional(|state| self::r#selection_set(state)))
                })
            })
        }
        #[inline]
        pub fn r#alias(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::Alias, |state| {
                state.sequence(|state| {
                    self::r#name(state)
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.match_string(":"))
                })
            })
        }
        #[inline]
        pub fn r#fragment_spread(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::FragmentSpread, |state| {
                state.sequence(|state| {
                    state
                        .match_string("...")
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.lookahead(false, |state| self::r#type_condition(state)))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| self::r#name(state))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.optional(|state| self::r#directives(state)))
                })
            })
        }
        #[inline]
        pub fn r#inline_fragment(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::InlineFragment, |state| {
                state.sequence(|state| {
                    state
                        .match_string("...")
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.optional(|state| self::r#type_condition(state)))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.optional(|state| self::r#directives(state)))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| self::r#selection_set(state))
                })
            })
        }
        #[inline]
        pub fn r#fragment_definition(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::FragmentDefinition, |state| {
                state.sequence(|state| {
                    state
                        .match_string("fragment")
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| self::r#name(state))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| self::r#type_condition(state))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.optional(|state| self::r#directives(state)))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| self::r#selection_set(state))
                })
            })
        }
        #[inline]
        pub fn r#type_condition(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.atomic(::plain_libs::peg::Atomicity::CompoundAtomic, |state| {
                state.rule(Rule::TypeCondition, |state| {
                    state.sequence(|state| {
                        state
                            .match_string("on")
                            .and_then(|state| {
                                state.sequence(|state| {
                                    self::whitespace(state)
                                        .and_then(|state| state.repeat(|state| self::whitespace(state)))
                                })
                            })
                            .and_then(|state| self::r#name(state))
                    })
                })
            })
        }
        #[inline]
        pub fn r#operation_type(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::OperationType, |state| {
                state
                    .match_string("query")
                    .or_else(|state| state.match_string("mutation"))
                    .or_else(|state| state.match_string("subscription"))
            })
        }
        #[inline]
        pub fn r#default_value(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::DefaultValue, |state| {
                state.sequence(|state| {
                    state
                        .match_string("=")
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| self::r#const_value(state))
                })
            })
        }
        #[inline]
        pub fn r#type_(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::Type, |state| {
                state.atomic(::plain_libs::peg::Atomicity::Atomic, |state| {
                    state.sequence(|state| {
                        self::r#name(state)
                            .or_else(|state| {
                                state.sequence(|state| {
                                    state
                                        .match_string("[")
                                        .and_then(|state| self::r#type_(state))
                                        .and_then(|state| state.match_string("]"))
                                })
                            })
                            .and_then(|state| state.optional(|state| state.match_string("!")))
                    })
                })
            })
        }
        #[inline]
        pub fn r#const_value(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::ConstValue, |state| {
                self::r#number(state)
                    .or_else(|state| self::r#string(state))
                    .or_else(|state| self::r#boolean(state))
                    .or_else(|state| self::r#null(state))
                    .or_else(|state| self::r#enum_value(state))
                    .or_else(|state| self::r#const_list(state))
                    .or_else(|state| self::r#const_object(state))
            })
        }
        #[inline]
        pub fn r#value(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::Value, |state| {
                self::r#variable(state)
                    .or_else(|state| self::r#number(state))
                    .or_else(|state| self::r#string(state))
                    .or_else(|state| self::r#boolean(state))
                    .or_else(|state| self::r#null(state))
                    .or_else(|state| self::r#enum_value(state))
                    .or_else(|state| self::r#list(state))
                    .or_else(|state| self::r#object(state))
            })
        }
        #[inline]
        pub fn r#variable(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::Variable, |state| {
                state.sequence(|state| {
                    state
                        .match_string("$")
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| self::r#name(state))
                })
            })
        }
        #[inline]
        pub fn r#number(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::Number, |state| {
                state.atomic(::plain_libs::peg::Atomicity::Atomic, |state| {
                    state.sequence(|state| {
                        self::r#float(state)
                            .or_else(|state| self::r#int(state))
                            .and_then(|state| state.lookahead(false, |state| self::r#name_start(state)))
                    })
                })
            })
        }
        #[inline]
        pub fn r#float(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::Float, |state| {
                state.sequence(|state| {
                    self::r#int(state)
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| {
                            state
                                .sequence(|state| {
                                    self::r#fractional(state)
                                        .and_then(|state| super::hidden::skip(state))
                                        .and_then(|state| self::r#exponent(state))
                                })
                                .or_else(|state| self::r#fractional(state))
                                .or_else(|state| self::r#exponent(state))
                        })
                })
            })
        }
        #[inline]
        pub fn r#fractional(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::Fractional, |state| {
                state.sequence(|state| {
                    state
                        .match_string(".")
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| self::r#ascii_digit(state))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| {
                            state.sequence(|state| {
                                state.optional(|state| {
                                    self::r#ascii_digit(state).and_then(|state| {
                                        state.repeat(|state| {
                                            state.sequence(|state| {
                                                super::hidden::skip(state)
                                                    .and_then(|state| self::r#ascii_digit(state))
                                            })
                                        })
                                    })
                                })
                            })
                        })
                })
            })
        }
        #[inline]
        pub fn r#exponent(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::Exponent, |state| {
                state.sequence(|state| {
                    state
                        .match_string("E")
                        .or_else(|state| state.match_string("e"))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| {
                            state.optional(|state| {
                                state
                                    .match_string("+")
                                    .or_else(|state| state.match_string("-"))
                            })
                        })
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| self::r#ascii_digit(state))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| {
                            state.sequence(|state| {
                                state.optional(|state| {
                                    self::r#ascii_digit(state).and_then(|state| {
                                        state.repeat(|state| {
                                            state.sequence(|state| {
                                                super::hidden::skip(state)
                                                    .and_then(|state| self::r#ascii_digit(state))
                                            })
                                        })
                                    })
                                })
                            })
                        })
                })
            })
        }
        # [inline] pub fn r#int (state : :: std :: boxed :: Box < :: plain_libs :: peg :: ParserState < '_ , Rule > >) -> :: plain_libs :: peg :: ParseResult < :: std :: boxed :: Box < :: plain_libs :: peg :: ParserState < '_ , Rule > > > { state . rule (Rule :: Int , | state | { state . sequence (| state | { state . optional (| state | { state . match_string ("-") }) . and_then (| state | { super :: hidden :: skip (state) }) . and_then (| state | { state . match_string ("0") . or_else (| state | { state . sequence (| state | { self :: r#ascii_nonzero_digit (state) . and_then (| state | { super :: hidden :: skip (state) }) . and_then (| state | { state . sequence (| state | { state . optional (| state | { self :: r#ascii_digit (state) . and_then (| state | { state . repeat (| state | { state . sequence (| state | { super :: hidden :: skip (state) . and_then (| state | { self :: r#ascii_digit (state) }) }) }) }) }) }) }) }) }) }) }) }) }
        #[inline]
        pub fn r#string(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.atomic(::plain_libs::peg::Atomicity::CompoundAtomic, |state| {
                state.rule(Rule::String, |state| {
                    state
                        .sequence(|state| {
                            state
                                .match_string("\"\"\"")
                                .and_then(|state| self::r#block_string_content(state))
                                .and_then(|state| state.match_string("\"\"\""))
                        })
                        .or_else(|state| {
                            state.sequence(|state| {
                                state
                                    .match_string("\"")
                                    .and_then(|state| self::r#string_content(state))
                                    .and_then(|state| state.match_string("\""))
                            })
                        })
                })
            })
        }
        #[inline]
        pub fn r#block_string_content(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::BlockStringContent, |state| {
                state.atomic(::plain_libs::peg::Atomicity::Atomic, |state| {
                    state.repeat(|state| self::r#block_string_character(state))
                })
            })
        }
        #[inline]
        pub fn r#block_string_character(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::BlockStringCharacter, |state| {
                state
                    .sequence(|state| {
                        state
                            .lookahead(false, |state| {
                                state
                                    .match_string("\"\"\"")
                                    .or_else(|state| state.match_string("\\\"\"\""))
                            })
                            .and_then(|state| super::hidden::skip(state))
                            .and_then(|state| self::r#any(state))
                    })
                    .or_else(|state| state.match_string("\\\"\"\""))
            })
        }
        #[inline]
        pub fn r#string_content(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::StringContent, |state| {
                state.atomic(::plain_libs::peg::Atomicity::Atomic, |state| {
                    state.repeat(|state| self::r#string_character(state))
                })
            })
        }
        #[inline]
        pub fn r#string_character(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::StringCharacter, |state| {
                state
                    .sequence(|state| {
                        state
                            .lookahead(false, |state| {
                                state
                                    .match_string("\"")
                                    .or_else(|state| state.match_string("\\"))
                                    .or_else(|state| self::r#line_terminator(state))
                            })
                            .and_then(|state| super::hidden::skip(state))
                            .and_then(|state| self::r#any(state))
                    })
                    .or_else(|state| {
                        state.sequence(|state| {
                            state
                                .match_string("\\")
                                .and_then(|state| super::hidden::skip(state))
                                .and_then(|state| {
                                    state
                                        .match_string("\"")
                                        .or_else(|state| state.match_string("\\"))
                                        .or_else(|state| state.match_string("/"))
                                        .or_else(|state| state.match_string("b"))
                                        .or_else(|state| state.match_string("f"))
                                        .or_else(|state| state.match_string("n"))
                                        .or_else(|state| state.match_string("r"))
                                        .or_else(|state| state.match_string("t"))
                                })
                        })
                    })
                    .or_else(|state| {
                        state.sequence(|state| {
                            state
                                .match_string("\\u")
                                .and_then(|state| super::hidden::skip(state))
                                .and_then(|state| self::r#unicode_scalar_value_hex(state))
                        })
                    })
            })
        }
        #[inline]
        pub fn r#unicode_scalar_value_hex(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::UnicodeScalarValueHex, |state| {
                state.sequence(|state| {
                    state
                        .lookahead(false, |state| {
                            state.sequence(|state| {
                                state
                                    .match_insensitive("d")
                                    .and_then(|state| super::hidden::skip(state))
                                    .and_then(|state| {
                                        state
                                            .match_range('8'..'9')
                                            .or_else(|state| state.match_range('a'..'f'))
                                            .or_else(|state| state.match_range('A'..'F'))
                                    })
                            })
                        })
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| self::r#ascii_hex_digit(state))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| self::r#ascii_hex_digit(state))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| self::r#ascii_hex_digit(state))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| self::r#ascii_hex_digit(state))
                })
            })
        }
        #[inline]
        pub fn r#boolean(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::Boolean, |state| {
                state
                    .match_string("true")
                    .or_else(|state| state.match_string("false"))
            })
        }
        #[inline]
        pub fn r#null(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::Null, |state| state.match_string("null"))
        }
        #[inline]
        pub fn r#enum_value(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.atomic(::plain_libs::peg::Atomicity::CompoundAtomic, |state| {
                state.rule(Rule::EnumValue, |state| {
                    state.sequence(|state| {
                        state
                            .lookahead(false, |state| {
                                self::r#boolean(state).or_else(|state| self::r#null(state))
                            })
                            .and_then(|state| self::r#name(state))
                    })
                })
            })
        }
        #[inline]
        pub fn r#const_list(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::ConstList, |state| {
                state.sequence(|state| {
                    state
                        .match_string("[")
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| {
                            state.sequence(|state| {
                                state.optional(|state| {
                                    self::r#const_value(state).and_then(|state| {
                                        state.repeat(|state| {
                                            state.sequence(|state| {
                                                super::hidden::skip(state)
                                                    .and_then(|state| self::r#const_value(state))
                                            })
                                        })
                                    })
                                })
                            })
                        })
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.match_string("]"))
                })
            })
        }
        #[inline]
        pub fn r#list(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::List, |state| {
                state.sequence(|state| {
                    state
                        .match_string("[")
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| {
                            state.sequence(|state| {
                                state.optional(|state| {
                                    self::r#value(state).and_then(|state| {
                                        state.repeat(|state| {
                                            state.sequence(|state| {
                                                super::hidden::skip(state)
                                                    .and_then(|state| self::r#value(state))
                                            })
                                        })
                                    })
                                })
                            })
                        })
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.match_string("]"))
                })
            })
        }
        #[inline]
        pub fn r#const_object(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::ConstObject, |state| {
                state.sequence(|state| {
                    state
                        .match_string("{")
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| {
                            state.sequence(|state| {
                                state.optional(|state| {
                                    self::r#const_object_field(state).and_then(|state| {
                                        state.repeat(|state| {
                                            state.sequence(|state| {
                                                super::hidden::skip(state)
                                                    .and_then(|state| self::r#const_object_field(state))
                                            })
                                        })
                                    })
                                })
                            })
                        })
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.match_string("}"))
                })
            })
        }
        #[inline]
        pub fn r#object(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::Object, |state| {
                state.sequence(|state| {
                    state
                        .match_string("{")
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| {
                            state.sequence(|state| {
                                state.optional(|state| {
                                    self::r#object_field(state).and_then(|state| {
                                        state.repeat(|state| {
                                            state.sequence(|state| {
                                                super::hidden::skip(state)
                                                    .and_then(|state| self::r#object_field(state))
                                            })
                                        })
                                    })
                                })
                            })
                        })
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.match_string("}"))
                })
            })
        }
        #[inline]
        pub fn r#const_object_field(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::ConstObjectField, |state| {
                state.sequence(|state| {
                    self::r#name(state)
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.match_string(":"))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| self::r#const_value(state))
                })
            })
        }
        #[inline]
        pub fn r#object_field(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::ObjectField, |state| {
                state.sequence(|state| {
                    self::r#name(state)
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.match_string(":"))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| self::r#value(state))
                })
            })
        }
        #[inline]
        pub fn r#directives(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::Directives, |state| {
                state.sequence(|state| {
                    self::r#directive(state)
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| {
                            state.sequence(|state| {
                                state.optional(|state| {
                                    self::r#directive(state).and_then(|state| {
                                        state.repeat(|state| {
                                            state.sequence(|state| {
                                                super::hidden::skip(state)
                                                    .and_then(|state| self::r#directive(state))
                                            })
                                        })
                                    })
                                })
                            })
                        })
                })
            })
        }
        #[inline]
        pub fn r#directive(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::Directive, |state| {
                state.sequence(|state| {
                    state
                        .match_string("@")
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| self::r#name(state))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.optional(|state| self::r#arguments(state)))
                })
            })
        }
        # [inline] pub fn r#arguments (state : :: std :: boxed :: Box < :: plain_libs :: peg :: ParserState < '_ , Rule > >) -> :: plain_libs :: peg :: ParseResult < :: std :: boxed :: Box < :: plain_libs :: peg :: ParserState < '_ , Rule > > > { state . rule (Rule :: Arguments , | state | { state . sequence (| state | { state . match_string ("(") . and_then (| state | { super :: hidden :: skip (state) }) . and_then (| state | { state . sequence (| state | { self :: r#argument (state) . and_then (| state | { super :: hidden :: skip (state) }) . and_then (| state | { state . sequence (| state | { state . optional (| state | { self :: r#argument (state) . and_then (| state | { state . repeat (| state | { state . sequence (| state | { super :: hidden :: skip (state) . and_then (| state | { self :: r#argument (state) }) }) }) }) }) }) }) }) }) . and_then (| state | { super :: hidden :: skip (state) }) . and_then (| state | { state . match_string (")") }) }) }) }
        #[inline]
        pub fn r#argument(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::Argument, |state| {
                state.sequence(|state| {
                    self::r#name(state)
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| state.match_string(":"))
                        .and_then(|state| super::hidden::skip(state))
                        .and_then(|state| self::r#value(state))
                })
            })
        }
        #[inline]
        pub fn r#name_start(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::NameStart, |state| {
                state.atomic(::plain_libs::peg::Atomicity::Atomic, |state| {
                    self::r#ascii_alpha(state).or_else(|state| state.match_string("_"))
                })
            })
        }
        #[inline]
        pub fn r#name(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::Name, |state| {
                state.atomic(::plain_libs::peg::Atomicity::Atomic, |state| {
                    state.sequence(|state| {
                        self::r#name_start(state).and_then(|state| {
                            state.repeat(|state| {
                                self::r#ascii_alpha(state)
                                    .or_else(|state| self::r#ascii_digit(state))
                                    .or_else(|state| state.match_string("_"))
                            })
                        })
                    })
                })
            })
        }
        #[inline]
        pub fn any(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.skip(1)
        }
        #[inline]
        pub fn eoi(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.rule(Rule::Eoi, |state| state.end_of_input())
        }
        #[inline]
        pub fn soi(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.start_of_input()
        }
        #[inline]
        pub fn ascii_digit(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.match_range('0'..'9')
        }
        #[inline]
        pub fn ascii_nonzero_digit(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state.match_range('1'..'9')
        }
        #[inline]
        pub fn ascii_hex_digit(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state
                .match_range('0'..'9')
                .or_else(|state| state.match_range('a'..'f'))
                .or_else(|state| state.match_range('A'..'F'))
        }
        #[inline]
        pub fn ascii_alpha(
            state: ::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>,
        ) -> ::plain_libs::peg::ParseResult<::std::boxed::Box<::plain_libs::peg::ParserState<'_, Rule>>> {
            state
                .match_range('a'..'z')
                .or_else(|state| state.match_range('A'..'Z'))
        }
    }
    pub use self::visible::*;
}
impl std::fmt::Debug for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Eoi => "EOI",
            Self::LineTerminator => "line_terminator",
            Self::ExecutableDocument => "executable_document",
            Self::ExecutableDefinition => "executable_definition",
            Self::OperationDefinition => "operation_definition",
            Self::NamedOperationDefinition => "named_operation_definition",
            Self::VariableDefinitions => "variable_definitions",
            Self::VariableDefinition => "variable_definition",
            Self::SelectionSet => "selection_set",
            Self::Selection => "selection",
            Self::Field => "field",
            Self::Alias => "alias",
            Self::FragmentSpread => "fragment_spread",
            Self::InlineFragment => "inline_fragment",
            Self::FragmentDefinition => "fragment_definition",
            Self::TypeCondition => "type_condition",
            Self::OperationType => "operation_type",
            Self::DefaultValue => "default_value",
            Self::Type => "type_",
            Self::ConstValue => "const_value",
            Self::Value => "value",
            Self::Variable => "variable",
            Self::Number => "number",
            Self::Float => "float",
            Self::Fractional => "fractional",
            Self::Exponent => "exponent",
            Self::Int => "int",
            Self::String => "string",
            Self::BlockStringContent => "block_string_content",
            Self::BlockStringCharacter => "block_string_character",
            Self::StringContent => "string_content",
            Self::StringCharacter => "string_character",
            Self::UnicodeScalarValueHex => "unicode_scalar_value_hex",
            Self::Boolean => "boolean",
            Self::Null => "null",
            Self::EnumValue => "enum_value",
            Self::ConstList => "const_list",
            Self::List => "list",
            Self::ConstObject => "const_object",
            Self::Object => "object",
            Self::ConstObjectField => "const_object_field",
            Self::ObjectField => "object_field",
            Self::Directives => "directives",
            Self::Directive => "directive",
            Self::Arguments => "arguments",
            Self::Argument => "argument",
            Self::NameStart => "name_start",
            Self::Name => "name",
        })
    }
}
