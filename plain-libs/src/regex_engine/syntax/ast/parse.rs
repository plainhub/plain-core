use crate::regex_engine::syntax::{
    ast::{self, Ast, Position, Span},
    either::Either,
    is_escapeable_character, is_meta_character,
};
use core::{
    borrow::Borrow,
    cell::{Cell, RefCell},
    mem,
};
use std::{
    boxed::Box,
    string::{String, ToString},
    vec,
    vec::Vec,
};
type Result<T> = core::result::Result<T, ast::Error>;
#[derive(Clone, Debug, Eq, PartialEq)]
enum Primitive {
    Literal(ast::Literal),
    Assertion(ast::Assertion),
    Dot(Span),
    Perl(ast::ClassPerl),
    Unicode(ast::ClassUnicode),
}
impl Primitive {
    fn span(&self) -> &Span {
        match *self {
            Primitive::Literal(ref x) => &x.span,
            Primitive::Assertion(ref x) => &x.span,
            Primitive::Dot(ref span) => span,
            Primitive::Perl(ref x) => &x.span,
            Primitive::Unicode(ref x) => &x.span,
        }
    }
    fn into_ast(self) -> Ast {
        match self {
            Primitive::Literal(lit) => Ast::literal(lit),
            Primitive::Assertion(assert) => Ast::assertion(assert),
            Primitive::Dot(span) => Ast::dot(span),
            Primitive::Perl(cls) => Ast::class_perl(cls),
            Primitive::Unicode(cls) => Ast::class_unicode(cls),
        }
    }
    fn into_class_set_item<P: Borrow<Parser>>(
        self,
        p: &ParserI<'_, P>,
    ) -> Result<ast::ClassSetItem> {
        use self::Primitive::*;
        use crate::regex_engine::syntax::ast::ClassSetItem;
        match self {
            Literal(lit) => Ok(ClassSetItem::Literal(lit)),
            Perl(cls) => Ok(ClassSetItem::Perl(cls)),
            Unicode(cls) => Ok(ClassSetItem::Unicode(cls)),
            x => Err(p.error(*x.span(), ast::ErrorKind::ClassEscapeInvalid)),
        }
    }
    fn into_class_literal<P: Borrow<Parser>>(self, p: &ParserI<'_, P>) -> Result<ast::Literal> {
        use self::Primitive::*;
        match self {
            Literal(lit) => Ok(lit),
            x => Err(p.error(*x.span(), ast::ErrorKind::ClassRangeLiteral)),
        }
    }
}
fn is_hex(c: char) -> bool {
    ('0' <= c && c <= '9') || ('a' <= c && c <= 'f') || ('A' <= c && c <= 'F')
}
fn is_capture_char(c: char, first: bool) -> bool {
    if first {
        c == '_' || c.is_alphabetic()
    } else {
        c == '_' || c == '.' || c == '[' || c == ']' || c.is_alphanumeric()
    }
}
#[derive(Clone, Debug)]
pub struct ParserBuilder {
    ignore_whitespace: bool,
    nest_limit: u32,
    octal: bool,
    empty_min_range: bool,
}
impl Default for ParserBuilder {
    fn default() -> ParserBuilder {
        ParserBuilder::new()
    }
}
impl ParserBuilder {
    pub fn new() -> ParserBuilder {
        ParserBuilder {
            ignore_whitespace: false,
            nest_limit: 250,
            octal: false,
            empty_min_range: false,
        }
    }
    pub fn build(&self) -> Parser {
        Parser {
            pos: Cell::new(Position {
                offset: 0,
                line: 1,
                column: 1,
            }),
            capture_index: Cell::new(0),
            nest_limit: self.nest_limit,
            octal: self.octal,
            empty_min_range: self.empty_min_range,
            initial_ignore_whitespace: self.ignore_whitespace,
            ignore_whitespace: Cell::new(self.ignore_whitespace),
            comments: RefCell::new(vec![]),
            stack_group: RefCell::new(vec![]),
            stack_class: RefCell::new(vec![]),
            capture_names: RefCell::new(vec![]),
            scratch: RefCell::new(String::new()),
        }
    }
    pub fn nest_limit(&mut self, limit: u32) -> &mut ParserBuilder {
        self.nest_limit = limit;
        self
    }
    pub fn octal(&mut self, yes: bool) -> &mut ParserBuilder {
        self.octal = yes;
        self
    }
    pub fn ignore_whitespace(&mut self, yes: bool) -> &mut ParserBuilder {
        self.ignore_whitespace = yes;
        self
    }
    pub fn empty_min_range(&mut self, yes: bool) -> &mut ParserBuilder {
        self.empty_min_range = yes;
        self
    }
}
#[derive(Clone, Debug)]
pub struct Parser {
    pos: Cell<Position>,
    capture_index: Cell<u32>,
    nest_limit: u32,
    octal: bool,
    initial_ignore_whitespace: bool,
    empty_min_range: bool,
    ignore_whitespace: Cell<bool>,
    comments: RefCell<Vec<ast::Comment>>,
    stack_group: RefCell<Vec<GroupState>>,
    stack_class: RefCell<Vec<ClassState>>,
    capture_names: RefCell<Vec<ast::CaptureName>>,
    scratch: RefCell<String>,
}
#[derive(Clone, Debug)]
struct ParserI<'s, P> {
    parser: P,
    pattern: &'s str,
}
#[derive(Clone, Debug)]
enum GroupState {
    Group {
        concat: ast::Concat,
        group: ast::Group,
        ignore_whitespace: bool,
    },
    Alternation(ast::Alternation),
}
#[derive(Clone, Debug)]
enum ClassState {
    Open {
        union: ast::ClassSetUnion,
        set: ast::ClassBracketed,
    },
    Op {
        kind: ast::ClassSetBinaryOpKind,
        lhs: ast::ClassSet,
    },
}
impl Parser {
    pub fn new() -> Parser {
        ParserBuilder::new().build()
    }
    pub fn parse(&mut self, pattern: &str) -> Result<Ast> {
        ParserI::new(self, pattern).parse()
    }
    pub fn parse_with_comments(&mut self, pattern: &str) -> Result<ast::WithComments> {
        ParserI::new(self, pattern).parse_with_comments()
    }
    fn reset(&self) {
        self.pos.set(Position {
            offset: 0,
            line: 1,
            column: 1,
        });
        self.ignore_whitespace.set(self.initial_ignore_whitespace);
        self.comments.borrow_mut().clear();
        self.stack_group.borrow_mut().clear();
        self.stack_class.borrow_mut().clear();
    }
}
impl<'s, P: Borrow<Parser>> ParserI<'s, P> {
    fn new(parser: P, pattern: &'s str) -> ParserI<'s, P> {
        ParserI { parser, pattern }
    }
    fn parser(&self) -> &Parser {
        self.parser.borrow()
    }
    fn pattern(&self) -> &str {
        self.pattern
    }
    fn error(&self, span: Span, kind: ast::ErrorKind) -> ast::Error {
        ast::Error {
            kind,
            pattern: self.pattern().to_string(),
            span,
        }
    }
    fn offset(&self) -> usize {
        self.parser().pos.get().offset
    }
    fn line(&self) -> usize {
        self.parser().pos.get().line
    }
    fn column(&self) -> usize {
        self.parser().pos.get().column
    }
    fn next_capture_index(&self, span: Span) -> Result<u32> {
        let current = self.parser().capture_index.get();
        let i = current
            .checked_add(1)
            .ok_or_else(|| self.error(span, ast::ErrorKind::CaptureLimitExceeded))?;
        self.parser().capture_index.set(i);
        Ok(i)
    }
    fn add_capture_name(&self, cap: &ast::CaptureName) -> Result<()> {
        let mut names = self.parser().capture_names.borrow_mut();
        match names.binary_search_by_key(&cap.name.as_str(), |c| c.name.as_str()) {
            Err(i) => {
                names.insert(i, cap.clone());
                Ok(())
            }
            Ok(i) => Err(self.error(
                cap.span,
                ast::ErrorKind::GroupNameDuplicate {
                    original: names[i].span,
                },
            )),
        }
    }
    fn ignore_whitespace(&self) -> bool {
        self.parser().ignore_whitespace.get()
    }
    fn char(&self) -> char {
        self.char_at(self.offset())
    }
    fn char_at(&self, i: usize) -> char {
        self.pattern()[i..]
            .chars()
            .next()
            .unwrap_or_else(|| panic!("expected char at offset {i}"))
    }
    fn bump(&self) -> bool {
        if self.is_eof() {
            return false;
        }
        let Position {
            mut offset,
            mut line,
            mut column,
        } = self.pos();
        if self.char() == '\n' {
            line = line.checked_add(1).unwrap();
            column = 1;
        } else {
            column = column.checked_add(1).unwrap();
        }
        offset += self.char().len_utf8();
        self.parser().pos.set(Position {
            offset,
            line,
            column,
        });
        self.pattern()[self.offset()..].chars().next().is_some()
    }
    fn bump_if(&self, prefix: &str) -> bool {
        if self.pattern()[self.offset()..].starts_with(prefix) {
            for _ in 0..prefix.chars().count() {
                self.bump();
            }
            true
        } else {
            false
        }
    }
    fn is_lookaround_prefix(&self) -> bool {
        self.bump_if("?=") || self.bump_if("?!") || self.bump_if("?<=") || self.bump_if("?<!")
    }
    fn bump_and_bump_space(&self) -> bool {
        if !self.bump() {
            return false;
        }
        self.bump_space();
        !self.is_eof()
    }
    fn bump_space(&self) {
        if !self.ignore_whitespace() {
            return;
        }
        while !self.is_eof() {
            if self.char().is_whitespace() {
                self.bump();
            } else if self.char() == '#' {
                let start = self.pos();
                let mut comment_text = String::new();
                self.bump();
                while !self.is_eof() {
                    let c = self.char();
                    self.bump();
                    if c == '\n' {
                        break;
                    }
                    comment_text.push(c);
                }
                let comment = ast::Comment {
                    span: Span::new(start, self.pos()),
                    comment: comment_text,
                };
                self.parser().comments.borrow_mut().push(comment);
            } else {
                break;
            }
        }
    }
    fn peek(&self) -> Option<char> {
        if self.is_eof() {
            return None;
        }
        self.pattern()[self.offset() + self.char().len_utf8()..]
            .chars()
            .next()
    }
    fn peek_space(&self) -> Option<char> {
        if !self.ignore_whitespace() {
            return self.peek();
        }
        if self.is_eof() {
            return None;
        }
        let mut start = self.offset() + self.char().len_utf8();
        let mut in_comment = false;
        for (i, c) in self.pattern()[start..].char_indices() {
            if c.is_whitespace() {
                continue;
            } else if !in_comment && c == '#' {
                in_comment = true;
            } else if in_comment && c == '\n' {
                in_comment = false;
            } else {
                start += i;
                break;
            }
        }
        self.pattern()[start..].chars().next()
    }
    fn is_eof(&self) -> bool {
        self.offset() == self.pattern().len()
    }
    fn pos(&self) -> Position {
        self.parser().pos.get()
    }
    fn span(&self) -> Span {
        Span::splat(self.pos())
    }
    fn span_char(&self) -> Span {
        let mut next = Position {
            offset: self.offset().checked_add(self.char().len_utf8()).unwrap(),
            line: self.line(),
            column: self.column().checked_add(1).unwrap(),
        };
        if self.char() == '\n' {
            next.line += 1;
            next.column = 1;
        }
        Span::new(self.pos(), next)
    }
    #[inline(never)]
    fn push_alternate(&self, mut concat: ast::Concat) -> Result<ast::Concat> {
        assert_eq!(self.char(), '|');
        concat.span.end = self.pos();
        self.push_or_add_alternation(concat);
        self.bump();
        Ok(ast::Concat {
            span: self.span(),
            asts: vec![],
        })
    }
    fn push_or_add_alternation(&self, concat: ast::Concat) {
        use self::GroupState::*;
        let mut stack = self.parser().stack_group.borrow_mut();
        if let Some(&mut Alternation(ref mut alts)) = stack.last_mut() {
            alts.asts.push(concat.into_ast());
            return;
        }
        stack.push(Alternation(ast::Alternation {
            span: Span::new(concat.span.start, self.pos()),
            asts: vec![concat.into_ast()],
        }));
    }
    #[inline(never)]
    fn push_group(&self, mut concat: ast::Concat) -> Result<ast::Concat> {
        assert_eq!(self.char(), '(');
        match self.parse_group()? {
            Either::Left(set) => {
                let ignore = set.flags.flag_state(ast::Flag::IgnoreWhitespace);
                if let Some(v) = ignore {
                    self.parser().ignore_whitespace.set(v);
                }
                concat.asts.push(Ast::flags(set));
                Ok(concat)
            }
            Either::Right(group) => {
                let old_ignore_whitespace = self.ignore_whitespace();
                let new_ignore_whitespace = group
                    .flags()
                    .and_then(|f| f.flag_state(ast::Flag::IgnoreWhitespace))
                    .unwrap_or(old_ignore_whitespace);
                self.parser()
                    .stack_group
                    .borrow_mut()
                    .push(GroupState::Group {
                        concat,
                        group,
                        ignore_whitespace: old_ignore_whitespace,
                    });
                self.parser().ignore_whitespace.set(new_ignore_whitespace);
                Ok(ast::Concat {
                    span: self.span(),
                    asts: vec![],
                })
            }
        }
    }
    #[inline(never)]
    fn pop_group(&self, mut group_concat: ast::Concat) -> Result<ast::Concat> {
        use self::GroupState::*;
        assert_eq!(self.char(), ')');
        let mut stack = self.parser().stack_group.borrow_mut();
        let (mut prior_concat, mut group, ignore_whitespace, alt) = match stack.pop() {
            Some(Group {
                concat,
                group,
                ignore_whitespace,
            }) => (concat, group, ignore_whitespace, None),
            Some(Alternation(alt)) => match stack.pop() {
                Some(Group {
                    concat,
                    group,
                    ignore_whitespace,
                }) => (concat, group, ignore_whitespace, Some(alt)),
                None | Some(Alternation(_)) => {
                    return Err(self.error(self.span_char(), ast::ErrorKind::GroupUnopened));
                }
            },
            None => {
                return Err(self.error(self.span_char(), ast::ErrorKind::GroupUnopened));
            }
        };
        self.parser().ignore_whitespace.set(ignore_whitespace);
        group_concat.span.end = self.pos();
        self.bump();
        group.span.end = self.pos();
        match alt {
            Some(mut alt) => {
                alt.span.end = group_concat.span.end;
                alt.asts.push(group_concat.into_ast());
                group.ast = Box::new(alt.into_ast());
            }
            None => {
                group.ast = Box::new(group_concat.into_ast());
            }
        }
        prior_concat.asts.push(Ast::group(group));
        Ok(prior_concat)
    }
    #[inline(never)]
    fn pop_group_end(&self, mut concat: ast::Concat) -> Result<Ast> {
        concat.span.end = self.pos();
        let mut stack = self.parser().stack_group.borrow_mut();
        let ast = match stack.pop() {
            None => Ok(concat.into_ast()),
            Some(GroupState::Alternation(mut alt)) => {
                alt.span.end = self.pos();
                alt.asts.push(concat.into_ast());
                Ok(Ast::alternation(alt))
            }
            Some(GroupState::Group { group, .. }) => {
                return Err(self.error(group.span, ast::ErrorKind::GroupUnclosed));
            }
        };
        match stack.pop() {
            None => ast,
            Some(GroupState::Alternation(_)) => {
                unreachable!()
            }
            Some(GroupState::Group { group, .. }) => {
                Err(self.error(group.span, ast::ErrorKind::GroupUnclosed))
            }
        }
    }
    #[inline(never)]
    fn push_class_open(&self, parent_union: ast::ClassSetUnion) -> Result<ast::ClassSetUnion> {
        assert_eq!(self.char(), '[');
        let (nested_set, nested_union) = self.parse_set_class_open()?;
        self.parser()
            .stack_class
            .borrow_mut()
            .push(ClassState::Open {
                union: parent_union,
                set: nested_set,
            });
        Ok(nested_union)
    }
    #[inline(never)]
    fn pop_class(
        &self,
        nested_union: ast::ClassSetUnion,
    ) -> Result<Either<ast::ClassSetUnion, ast::ClassBracketed>> {
        assert_eq!(self.char(), ']');
        let item = ast::ClassSet::Item(nested_union.into_item());
        let prevset = self.pop_class_op(item);
        let mut stack = self.parser().stack_class.borrow_mut();
        match stack.pop() {
            None => {
                panic!("unexpected empty character class stack")
            }
            Some(ClassState::Op { .. }) => {
                panic!("unexpected ClassState::Op")
            }
            Some(ClassState::Open { mut union, mut set }) => {
                self.bump();
                set.span.end = self.pos();
                set.kind = prevset;
                if stack.is_empty() {
                    Ok(Either::Right(set))
                } else {
                    union.push(ast::ClassSetItem::Bracketed(Box::new(set)));
                    Ok(Either::Left(union))
                }
            }
        }
    }
    #[inline(never)]
    fn unclosed_class_error(&self) -> ast::Error {
        for state in self.parser().stack_class.borrow().iter().rev() {
            if let ClassState::Open { ref set, .. } = *state {
                return self.error(set.span, ast::ErrorKind::ClassUnclosed);
            }
        }
        panic!("no open character class found")
    }
    #[inline(never)]
    fn push_class_op(
        &self,
        next_kind: ast::ClassSetBinaryOpKind,
        next_union: ast::ClassSetUnion,
    ) -> ast::ClassSetUnion {
        let item = ast::ClassSet::Item(next_union.into_item());
        let new_lhs = self.pop_class_op(item);
        self.parser().stack_class.borrow_mut().push(ClassState::Op {
            kind: next_kind,
            lhs: new_lhs,
        });
        ast::ClassSetUnion {
            span: self.span(),
            items: vec![],
        }
    }
    #[inline(never)]
    fn pop_class_op(&self, rhs: ast::ClassSet) -> ast::ClassSet {
        let mut stack = self.parser().stack_class.borrow_mut();
        let (kind, lhs) = match stack.pop() {
            Some(ClassState::Op { kind, lhs }) => (kind, lhs),
            Some(state @ ClassState::Open { .. }) => {
                stack.push(state);
                return rhs;
            }
            None => unreachable!(),
        };
        let span = Span::new(lhs.span().start, rhs.span().end);
        ast::ClassSet::BinaryOp(ast::ClassSetBinaryOp {
            span,
            kind,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        })
    }
}
impl<'s, P: Borrow<Parser>> ParserI<'s, P> {
    fn parse(&self) -> Result<Ast> {
        self.parse_with_comments().map(|astc| astc.ast)
    }
    fn parse_with_comments(&self) -> Result<ast::WithComments> {
        assert_eq!(self.offset(), 0, "parser can only be used once");
        self.parser().reset();
        let mut concat = ast::Concat {
            span: self.span(),
            asts: vec![],
        };
        loop {
            self.bump_space();
            if self.is_eof() {
                break;
            }
            match self.char() {
                '(' => concat = self.push_group(concat)?,
                ')' => concat = self.pop_group(concat)?,
                '|' => concat = self.push_alternate(concat)?,
                '[' => {
                    let class = self.parse_set_class()?;
                    concat.asts.push(Ast::class_bracketed(class));
                }
                '?' => {
                    concat =
                        self.parse_uncounted_repetition(concat, ast::RepetitionKind::ZeroOrOne)?;
                }
                '*' => {
                    concat =
                        self.parse_uncounted_repetition(concat, ast::RepetitionKind::ZeroOrMore)?;
                }
                '+' => {
                    concat =
                        self.parse_uncounted_repetition(concat, ast::RepetitionKind::OneOrMore)?;
                }
                '{' => {
                    concat = self.parse_counted_repetition(concat)?;
                }
                _ => concat.asts.push(self.parse_primitive()?.into_ast()),
            }
        }
        let ast = self.pop_group_end(concat)?;
        NestLimiter::new(self).check(&ast)?;
        Ok(ast::WithComments {
            ast,
            comments: mem::replace(&mut *self.parser().comments.borrow_mut(), vec![]),
        })
    }
    #[inline(never)]
    fn parse_uncounted_repetition(
        &self,
        mut concat: ast::Concat,
        kind: ast::RepetitionKind,
    ) -> Result<ast::Concat> {
        assert!(self.char() == '?' || self.char() == '*' || self.char() == '+');
        let op_start = self.pos();
        let ast = match concat.asts.pop() {
            Some(ast) => ast,
            None => return Err(self.error(self.span(), ast::ErrorKind::RepetitionMissing)),
        };
        match ast {
            Ast::Empty(_) | Ast::Flags(_) => {
                return Err(self.error(self.span(), ast::ErrorKind::RepetitionMissing));
            }
            _ => {}
        }
        let mut greedy = true;
        if self.bump() && self.char() == '?' {
            greedy = false;
            self.bump();
        }
        concat.asts.push(Ast::repetition(ast::Repetition {
            span: ast.span().with_end(self.pos()),
            op: ast::RepetitionOp {
                span: Span::new(op_start, self.pos()),
                kind,
            },
            greedy,
            ast: Box::new(ast),
        }));
        Ok(concat)
    }
    #[inline(never)]
    fn parse_counted_repetition(&self, mut concat: ast::Concat) -> Result<ast::Concat> {
        assert!(self.char() == '{');
        let start = self.pos();
        let ast = match concat.asts.pop() {
            Some(ast) => ast,
            None => return Err(self.error(self.span(), ast::ErrorKind::RepetitionMissing)),
        };
        match ast {
            Ast::Empty(_) | Ast::Flags(_) => {
                return Err(self.error(self.span(), ast::ErrorKind::RepetitionMissing));
            }
            _ => {}
        }
        if !self.bump_and_bump_space() {
            return Err(self.error(
                Span::new(start, self.pos()),
                ast::ErrorKind::RepetitionCountUnclosed,
            ));
        }
        let count_start = specialize_err(
            self.parse_decimal(),
            ast::ErrorKind::DecimalEmpty,
            ast::ErrorKind::RepetitionCountDecimalEmpty,
        );
        if self.is_eof() {
            return Err(self.error(
                Span::new(start, self.pos()),
                ast::ErrorKind::RepetitionCountUnclosed,
            ));
        }
        let range = if self.char() == ',' {
            if !self.bump_and_bump_space() {
                return Err(self.error(
                    Span::new(start, self.pos()),
                    ast::ErrorKind::RepetitionCountUnclosed,
                ));
            }
            if self.char() != '}' {
                let count_start = match count_start {
                    Ok(c) => c,
                    Err(err) if err.kind == ast::ErrorKind::RepetitionCountDecimalEmpty => {
                        if self.parser().empty_min_range {
                            0
                        } else {
                            return Err(err);
                        }
                    }
                    err => err?,
                };
                let count_end = specialize_err(
                    self.parse_decimal(),
                    ast::ErrorKind::DecimalEmpty,
                    ast::ErrorKind::RepetitionCountDecimalEmpty,
                )?;
                ast::RepetitionRange::Bounded(count_start, count_end)
            } else {
                ast::RepetitionRange::AtLeast(count_start?)
            }
        } else {
            ast::RepetitionRange::Exactly(count_start?)
        };
        if self.is_eof() || self.char() != '}' {
            return Err(self.error(
                Span::new(start, self.pos()),
                ast::ErrorKind::RepetitionCountUnclosed,
            ));
        }
        let mut greedy = true;
        if self.bump_and_bump_space() && self.char() == '?' {
            greedy = false;
            self.bump();
        }
        let op_span = Span::new(start, self.pos());
        if !range.is_valid() {
            return Err(self.error(op_span, ast::ErrorKind::RepetitionCountInvalid));
        }
        concat.asts.push(Ast::repetition(ast::Repetition {
            span: ast.span().with_end(self.pos()),
            op: ast::RepetitionOp {
                span: op_span,
                kind: ast::RepetitionKind::Range(range),
            },
            greedy,
            ast: Box::new(ast),
        }));
        Ok(concat)
    }
    #[inline(never)]
    fn parse_group(&self) -> Result<Either<ast::SetFlags, ast::Group>> {
        assert_eq!(self.char(), '(');
        let open_span = self.span_char();
        self.bump();
        self.bump_space();
        if self.is_lookaround_prefix() {
            return Err(self.error(
                Span::new(open_span.start, self.span().end),
                ast::ErrorKind::UnsupportedLookAround,
            ));
        }
        let inner_span = self.span();
        let mut starts_with_p = true;
        if self.bump_if("?P<") || {
            starts_with_p = false;
            self.bump_if("?<")
        } {
            let capture_index = self.next_capture_index(open_span)?;
            let name = self.parse_capture_name(capture_index)?;
            Ok(Either::Right(ast::Group {
                span: open_span,
                kind: ast::GroupKind::CaptureName {
                    starts_with_p,
                    name,
                },
                ast: Box::new(Ast::empty(self.span())),
            }))
        } else if self.bump_if("?") {
            if self.is_eof() {
                return Err(self.error(open_span, ast::ErrorKind::GroupUnclosed));
            }
            let flags = self.parse_flags()?;
            let char_end = self.char();
            self.bump();
            if char_end == ')' {
                if flags.items.is_empty() {
                    return Err(self.error(inner_span, ast::ErrorKind::RepetitionMissing));
                }
                Ok(Either::Left(ast::SetFlags {
                    span: Span {
                        end: self.pos(),
                        ..open_span
                    },
                    flags,
                }))
            } else {
                assert_eq!(char_end, ':');
                Ok(Either::Right(ast::Group {
                    span: open_span,
                    kind: ast::GroupKind::NonCapturing(flags),
                    ast: Box::new(Ast::empty(self.span())),
                }))
            }
        } else {
            let capture_index = self.next_capture_index(open_span)?;
            Ok(Either::Right(ast::Group {
                span: open_span,
                kind: ast::GroupKind::CaptureIndex(capture_index),
                ast: Box::new(Ast::empty(self.span())),
            }))
        }
    }
    #[inline(never)]
    fn parse_capture_name(&self, capture_index: u32) -> Result<ast::CaptureName> {
        if self.is_eof() {
            return Err(self.error(self.span(), ast::ErrorKind::GroupNameUnexpectedEof));
        }
        let start = self.pos();
        loop {
            if self.char() == '>' {
                break;
            }
            if !is_capture_char(self.char(), self.pos() == start) {
                return Err(self.error(self.span_char(), ast::ErrorKind::GroupNameInvalid));
            }
            if !self.bump() {
                break;
            }
        }
        let end = self.pos();
        if self.is_eof() {
            return Err(self.error(self.span(), ast::ErrorKind::GroupNameUnexpectedEof));
        }
        assert_eq!(self.char(), '>');
        self.bump();
        let name = &self.pattern()[start.offset..end.offset];
        if name.is_empty() {
            return Err(self.error(Span::new(start, start), ast::ErrorKind::GroupNameEmpty));
        }
        let capname = ast::CaptureName {
            span: Span::new(start, end),
            name: name.to_string(),
            index: capture_index,
        };
        self.add_capture_name(&capname)?;
        Ok(capname)
    }
    #[inline(never)]
    fn parse_flags(&self) -> Result<ast::Flags> {
        let mut flags = ast::Flags {
            span: self.span(),
            items: vec![],
        };
        let mut last_was_negation = None;
        while self.char() != ':' && self.char() != ')' {
            if self.char() == '-' {
                last_was_negation = Some(self.span_char());
                let item = ast::FlagsItem {
                    span: self.span_char(),
                    kind: ast::FlagsItemKind::Negation,
                };
                if let Some(i) = flags.add_item(item) {
                    return Err(self.error(
                        self.span_char(),
                        ast::ErrorKind::FlagRepeatedNegation {
                            original: flags.items[i].span,
                        },
                    ));
                }
            } else {
                last_was_negation = None;
                let item = ast::FlagsItem {
                    span: self.span_char(),
                    kind: ast::FlagsItemKind::Flag(self.parse_flag()?),
                };
                if let Some(i) = flags.add_item(item) {
                    return Err(self.error(
                        self.span_char(),
                        ast::ErrorKind::FlagDuplicate {
                            original: flags.items[i].span,
                        },
                    ));
                }
            }
            if !self.bump() {
                return Err(self.error(self.span(), ast::ErrorKind::FlagUnexpectedEof));
            }
        }
        if let Some(span) = last_was_negation {
            return Err(self.error(span, ast::ErrorKind::FlagDanglingNegation));
        }
        flags.span.end = self.pos();
        Ok(flags)
    }
    #[inline(never)]
    fn parse_flag(&self) -> Result<ast::Flag> {
        match self.char() {
            'i' => Ok(ast::Flag::CaseInsensitive),
            'm' => Ok(ast::Flag::MultiLine),
            's' => Ok(ast::Flag::DotMatchesNewLine),
            'U' => Ok(ast::Flag::SwapGreed),
            'u' => Ok(ast::Flag::Unicode),
            'R' => Ok(ast::Flag::CRLF),
            'x' => Ok(ast::Flag::IgnoreWhitespace),
            _ => Err(self.error(self.span_char(), ast::ErrorKind::FlagUnrecognized)),
        }
    }
    fn parse_primitive(&self) -> Result<Primitive> {
        match self.char() {
            '\\' => self.parse_escape(),
            '.' => {
                let ast = Primitive::Dot(self.span_char());
                self.bump();
                Ok(ast)
            }
            '^' => {
                let ast = Primitive::Assertion(ast::Assertion {
                    span: self.span_char(),
                    kind: ast::AssertionKind::StartLine,
                });
                self.bump();
                Ok(ast)
            }
            '$' => {
                let ast = Primitive::Assertion(ast::Assertion {
                    span: self.span_char(),
                    kind: ast::AssertionKind::EndLine,
                });
                self.bump();
                Ok(ast)
            }
            c => {
                let ast = Primitive::Literal(ast::Literal {
                    span: self.span_char(),
                    kind: ast::LiteralKind::Verbatim,
                    c,
                });
                self.bump();
                Ok(ast)
            }
        }
    }
    #[inline(never)]
    fn parse_escape(&self) -> Result<Primitive> {
        assert_eq!(self.char(), '\\');
        let start = self.pos();
        if !self.bump() {
            return Err(self.error(
                Span::new(start, self.pos()),
                ast::ErrorKind::EscapeUnexpectedEof,
            ));
        }
        let c = self.char();
        match c {
            '0'..='7' => {
                if !self.parser().octal {
                    return Err(self.error(
                        Span::new(start, self.span_char().end),
                        ast::ErrorKind::UnsupportedBackreference,
                    ));
                }
                let mut lit = self.parse_octal();
                lit.span.start = start;
                return Ok(Primitive::Literal(lit));
            }
            '8'..='9' if !self.parser().octal => {
                return Err(self.error(
                    Span::new(start, self.span_char().end),
                    ast::ErrorKind::UnsupportedBackreference,
                ));
            }
            'x' | 'u' | 'U' => {
                let mut lit = self.parse_hex()?;
                lit.span.start = start;
                return Ok(Primitive::Literal(lit));
            }
            'p' | 'P' => {
                let mut cls = self.parse_unicode_class()?;
                cls.span.start = start;
                return Ok(Primitive::Unicode(cls));
            }
            'd' | 's' | 'w' | 'D' | 'S' | 'W' => {
                let mut cls = self.parse_perl_class();
                cls.span.start = start;
                return Ok(Primitive::Perl(cls));
            }
            _ => {}
        }
        self.bump();
        let span = Span::new(start, self.pos());
        if is_meta_character(c) {
            return Ok(Primitive::Literal(ast::Literal {
                span,
                kind: ast::LiteralKind::Meta,
                c,
            }));
        }
        if is_escapeable_character(c) {
            return Ok(Primitive::Literal(ast::Literal {
                span,
                kind: ast::LiteralKind::Superfluous,
                c,
            }));
        }
        let special = |kind, c| {
            Ok(Primitive::Literal(ast::Literal {
                span,
                kind: ast::LiteralKind::Special(kind),
                c,
            }))
        };
        match c {
            'a' => special(ast::SpecialLiteralKind::Bell, '\x07'),
            'f' => special(ast::SpecialLiteralKind::FormFeed, '\x0C'),
            't' => special(ast::SpecialLiteralKind::Tab, '\t'),
            'n' => special(ast::SpecialLiteralKind::LineFeed, '\n'),
            'r' => special(ast::SpecialLiteralKind::CarriageReturn, '\r'),
            'v' => special(ast::SpecialLiteralKind::VerticalTab, '\x0B'),
            'A' => Ok(Primitive::Assertion(ast::Assertion {
                span,
                kind: ast::AssertionKind::StartText,
            })),
            'z' => Ok(Primitive::Assertion(ast::Assertion {
                span,
                kind: ast::AssertionKind::EndText,
            })),
            'b' => {
                let mut wb = ast::Assertion {
                    span,
                    kind: ast::AssertionKind::WordBoundary,
                };
                if !self.is_eof() && self.char() == '{' {
                    if let Some(kind) = self.maybe_parse_special_word_boundary(start)? {
                        wb.kind = kind;
                        wb.span.end = self.pos();
                    }
                }
                Ok(Primitive::Assertion(wb))
            }
            'B' => Ok(Primitive::Assertion(ast::Assertion {
                span,
                kind: ast::AssertionKind::NotWordBoundary,
            })),
            '<' => Ok(Primitive::Assertion(ast::Assertion {
                span,
                kind: ast::AssertionKind::WordBoundaryStartAngle,
            })),
            '>' => Ok(Primitive::Assertion(ast::Assertion {
                span,
                kind: ast::AssertionKind::WordBoundaryEndAngle,
            })),
            _ => Err(self.error(span, ast::ErrorKind::EscapeUnrecognized)),
        }
    }
    fn maybe_parse_special_word_boundary(
        &self,
        wb_start: Position,
    ) -> Result<Option<ast::AssertionKind>> {
        assert_eq!(self.char(), '{');
        let is_valid_char = |c| match c {
            'A'..='Z' | 'a'..='z' | '-' => true,
            _ => false,
        };
        let start = self.pos();
        if !self.bump_and_bump_space() {
            return Err(self.error(
                Span::new(wb_start, self.pos()),
                ast::ErrorKind::SpecialWordOrRepetitionUnexpectedEof,
            ));
        }
        let start_contents = self.pos();
        if !is_valid_char(self.char()) {
            self.parser().pos.set(start);
            return Ok(None);
        }
        let mut scratch = self.parser().scratch.borrow_mut();
        scratch.clear();
        while !self.is_eof() && is_valid_char(self.char()) {
            scratch.push(self.char());
            self.bump_and_bump_space();
        }
        if self.is_eof() || self.char() != '}' {
            return Err(self.error(
                Span::new(start, self.pos()),
                ast::ErrorKind::SpecialWordBoundaryUnclosed,
            ));
        }
        let end = self.pos();
        self.bump();
        let kind = match scratch.as_str() {
            "start" => ast::AssertionKind::WordBoundaryStart,
            "end" => ast::AssertionKind::WordBoundaryEnd,
            "start-half" => ast::AssertionKind::WordBoundaryStartHalf,
            "end-half" => ast::AssertionKind::WordBoundaryEndHalf,
            _ => {
                return Err(self.error(
                    Span::new(start_contents, end),
                    ast::ErrorKind::SpecialWordBoundaryUnrecognized,
                ));
            }
        };
        Ok(Some(kind))
    }
    #[inline(never)]
    fn parse_octal(&self) -> ast::Literal {
        assert!(self.parser().octal);
        assert!('0' <= self.char() && self.char() <= '7');
        let start = self.pos();
        while self.bump()
            && '0' <= self.char()
            && self.char() <= '7'
            && self.pos().offset - start.offset <= 2
        {}
        let end = self.pos();
        let octal = &self.pattern()[start.offset..end.offset];
        let codepoint = u32::from_str_radix(octal, 8).expect("valid octal number");
        let c = char::from_u32(codepoint).expect("Unicode scalar value");
        ast::Literal {
            span: Span::new(start, end),
            kind: ast::LiteralKind::Octal,
            c,
        }
    }
    #[inline(never)]
    fn parse_hex(&self) -> Result<ast::Literal> {
        assert!(self.char() == 'x' || self.char() == 'u' || self.char() == 'U');
        let hex_kind = match self.char() {
            'x' => ast::HexLiteralKind::X,
            'u' => ast::HexLiteralKind::UnicodeShort,
            _ => ast::HexLiteralKind::UnicodeLong,
        };
        if !self.bump_and_bump_space() {
            return Err(self.error(self.span(), ast::ErrorKind::EscapeUnexpectedEof));
        }
        if self.char() == '{' {
            self.parse_hex_brace(hex_kind)
        } else {
            self.parse_hex_digits(hex_kind)
        }
    }
    #[inline(never)]
    fn parse_hex_digits(&self, kind: ast::HexLiteralKind) -> Result<ast::Literal> {
        let mut scratch = self.parser().scratch.borrow_mut();
        scratch.clear();
        let start = self.pos();
        for i in 0..kind.digits() {
            if i > 0 && !self.bump_and_bump_space() {
                return Err(self.error(self.span(), ast::ErrorKind::EscapeUnexpectedEof));
            }
            if !is_hex(self.char()) {
                return Err(self.error(self.span_char(), ast::ErrorKind::EscapeHexInvalidDigit));
            }
            scratch.push(self.char());
        }
        self.bump_and_bump_space();
        let end = self.pos();
        let hex = scratch.as_str();
        match u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
            None => Err(self.error(Span::new(start, end), ast::ErrorKind::EscapeHexInvalid)),
            Some(c) => Ok(ast::Literal {
                span: Span::new(start, end),
                kind: ast::LiteralKind::HexFixed(kind),
                c,
            }),
        }
    }
    #[inline(never)]
    fn parse_hex_brace(&self, kind: ast::HexLiteralKind) -> Result<ast::Literal> {
        let mut scratch = self.parser().scratch.borrow_mut();
        scratch.clear();
        let brace_pos = self.pos();
        let start = self.span_char().end;
        while self.bump_and_bump_space() && self.char() != '}' {
            if !is_hex(self.char()) {
                return Err(self.error(self.span_char(), ast::ErrorKind::EscapeHexInvalidDigit));
            }
            scratch.push(self.char());
        }
        if self.is_eof() {
            return Err(self.error(
                Span::new(brace_pos, self.pos()),
                ast::ErrorKind::EscapeUnexpectedEof,
            ));
        }
        let end = self.pos();
        let hex = scratch.as_str();
        assert_eq!(self.char(), '}');
        self.bump_and_bump_space();
        if hex.is_empty() {
            return Err(self.error(
                Span::new(brace_pos, self.pos()),
                ast::ErrorKind::EscapeHexEmpty,
            ));
        }
        match u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
            None => Err(self.error(Span::new(start, end), ast::ErrorKind::EscapeHexInvalid)),
            Some(c) => Ok(ast::Literal {
                span: Span::new(start, self.pos()),
                kind: ast::LiteralKind::HexBrace(kind),
                c,
            }),
        }
    }
    fn parse_decimal(&self) -> Result<u32> {
        let mut scratch = self.parser().scratch.borrow_mut();
        scratch.clear();
        while !self.is_eof() && self.char().is_whitespace() {
            self.bump();
        }
        let start = self.pos();
        while !self.is_eof() && '0' <= self.char() && self.char() <= '9' {
            scratch.push(self.char());
            self.bump_and_bump_space();
        }
        let span = Span::new(start, self.pos());
        while !self.is_eof() && self.char().is_whitespace() {
            self.bump_and_bump_space();
        }
        let digits = scratch.as_str();
        if digits.is_empty() {
            return Err(self.error(span, ast::ErrorKind::DecimalEmpty));
        }
        match u32::from_str_radix(digits, 10).ok() {
            Some(n) => Ok(n),
            None => Err(self.error(span, ast::ErrorKind::DecimalInvalid)),
        }
    }
    #[inline(never)]
    fn parse_set_class(&self) -> Result<ast::ClassBracketed> {
        assert_eq!(self.char(), '[');
        let mut union = ast::ClassSetUnion {
            span: self.span(),
            items: vec![],
        };
        loop {
            self.bump_space();
            if self.is_eof() {
                return Err(self.unclosed_class_error());
            }
            match self.char() {
                '[' => {
                    if !self.parser().stack_class.borrow().is_empty() {
                        if let Some(cls) = self.maybe_parse_ascii_class() {
                            union.push(ast::ClassSetItem::Ascii(cls));
                            continue;
                        }
                    }
                    union = self.push_class_open(union)?;
                }
                ']' => match self.pop_class(union)? {
                    Either::Left(nested_union) => {
                        union = nested_union;
                    }
                    Either::Right(class) => return Ok(class),
                },
                '&' if self.peek() == Some('&') => {
                    assert!(self.bump_if("&&"));
                    union = self.push_class_op(ast::ClassSetBinaryOpKind::Intersection, union);
                }
                '-' if self.peek() == Some('-') => {
                    assert!(self.bump_if("--"));
                    union = self.push_class_op(ast::ClassSetBinaryOpKind::Difference, union);
                }
                '~' if self.peek() == Some('~') => {
                    assert!(self.bump_if("~~"));
                    union =
                        self.push_class_op(ast::ClassSetBinaryOpKind::SymmetricDifference, union);
                }
                _ => {
                    union.push(self.parse_set_class_range()?);
                }
            }
        }
    }
    #[inline(never)]
    fn parse_set_class_range(&self) -> Result<ast::ClassSetItem> {
        let prim1 = self.parse_set_class_item()?;
        self.bump_space();
        if self.is_eof() {
            return Err(self.unclosed_class_error());
        }
        if self.char() != '-' || self.peek_space() == Some(']') || self.peek_space() == Some('-') {
            return prim1.into_class_set_item(self);
        }
        if !self.bump_and_bump_space() {
            return Err(self.unclosed_class_error());
        }
        let prim2 = self.parse_set_class_item()?;
        let range = ast::ClassSetRange {
            span: Span::new(prim1.span().start, prim2.span().end),
            start: prim1.into_class_literal(self)?,
            end: prim2.into_class_literal(self)?,
        };
        if !range.is_valid() {
            return Err(self.error(range.span, ast::ErrorKind::ClassRangeInvalid));
        }
        Ok(ast::ClassSetItem::Range(range))
    }
    #[inline(never)]
    fn parse_set_class_item(&self) -> Result<Primitive> {
        if self.char() == '\\' {
            self.parse_escape()
        } else {
            let x = Primitive::Literal(ast::Literal {
                span: self.span_char(),
                kind: ast::LiteralKind::Verbatim,
                c: self.char(),
            });
            self.bump();
            Ok(x)
        }
    }
    #[inline(never)]
    fn parse_set_class_open(&self) -> Result<(ast::ClassBracketed, ast::ClassSetUnion)> {
        assert_eq!(self.char(), '[');
        let start = self.pos();
        if !self.bump_and_bump_space() {
            return Err(self.error(Span::new(start, self.pos()), ast::ErrorKind::ClassUnclosed));
        }
        let negated = if self.char() != '^' {
            false
        } else {
            if !self.bump_and_bump_space() {
                return Err(self.error(Span::new(start, self.pos()), ast::ErrorKind::ClassUnclosed));
            }
            true
        };
        let mut union = ast::ClassSetUnion {
            span: self.span(),
            items: vec![],
        };
        while self.char() == '-' {
            union.push(ast::ClassSetItem::Literal(ast::Literal {
                span: self.span_char(),
                kind: ast::LiteralKind::Verbatim,
                c: '-',
            }));
            if !self.bump_and_bump_space() {
                return Err(self.error(Span::new(start, start), ast::ErrorKind::ClassUnclosed));
            }
        }
        if union.items.is_empty() && self.char() == ']' {
            union.push(ast::ClassSetItem::Literal(ast::Literal {
                span: self.span_char(),
                kind: ast::LiteralKind::Verbatim,
                c: ']',
            }));
            if !self.bump_and_bump_space() {
                return Err(self.error(Span::new(start, self.pos()), ast::ErrorKind::ClassUnclosed));
            }
        }
        let set = ast::ClassBracketed {
            span: Span::new(start, self.pos()),
            negated,
            kind: ast::ClassSet::union(ast::ClassSetUnion {
                span: Span::new(union.span.start, union.span.start),
                items: vec![],
            }),
        };
        Ok((set, union))
    }
    #[inline(never)]
    fn maybe_parse_ascii_class(&self) -> Option<ast::ClassAscii> {
        assert_eq!(self.char(), '[');
        let start = self.pos();
        let mut negated = false;
        if !self.bump() || self.char() != ':' {
            self.parser().pos.set(start);
            return None;
        }
        if !self.bump() {
            self.parser().pos.set(start);
            return None;
        }
        if self.char() == '^' {
            negated = true;
            if !self.bump() {
                self.parser().pos.set(start);
                return None;
            }
        }
        let name_start = self.offset();
        while self.char() != ':' && self.bump() {}
        if self.is_eof() {
            self.parser().pos.set(start);
            return None;
        }
        let name = &self.pattern()[name_start..self.offset()];
        if !self.bump_if(":]") {
            self.parser().pos.set(start);
            return None;
        }
        let kind = match ast::ClassAsciiKind::from_name(name) {
            Some(kind) => kind,
            None => {
                self.parser().pos.set(start);
                return None;
            }
        };
        Some(ast::ClassAscii {
            span: Span::new(start, self.pos()),
            kind,
            negated,
        })
    }
    #[inline(never)]
    fn parse_unicode_class(&self) -> Result<ast::ClassUnicode> {
        assert!(self.char() == 'p' || self.char() == 'P');
        let mut scratch = self.parser().scratch.borrow_mut();
        scratch.clear();
        let negated = self.char() == 'P';
        if !self.bump_and_bump_space() {
            return Err(self.error(self.span(), ast::ErrorKind::EscapeUnexpectedEof));
        }
        let (start, kind) = if self.char() == '{' {
            let start = self.span_char().end;
            while self.bump_and_bump_space() && self.char() != '}' {
                scratch.push(self.char());
            }
            if self.is_eof() {
                return Err(self.error(self.span(), ast::ErrorKind::EscapeUnexpectedEof));
            }
            assert_eq!(self.char(), '}');
            self.bump();
            let name = scratch.as_str();
            if let Some(i) = name.find("!=") {
                (
                    start,
                    ast::ClassUnicodeKind::NamedValue {
                        op: ast::ClassUnicodeOpKind::NotEqual,
                        name: name[..i].to_string(),
                        value: name[i + 2..].to_string(),
                    },
                )
            } else if let Some(i) = name.find(':') {
                (
                    start,
                    ast::ClassUnicodeKind::NamedValue {
                        op: ast::ClassUnicodeOpKind::Colon,
                        name: name[..i].to_string(),
                        value: name[i + 1..].to_string(),
                    },
                )
            } else if let Some(i) = name.find('=') {
                (
                    start,
                    ast::ClassUnicodeKind::NamedValue {
                        op: ast::ClassUnicodeOpKind::Equal,
                        name: name[..i].to_string(),
                        value: name[i + 1..].to_string(),
                    },
                )
            } else {
                (start, ast::ClassUnicodeKind::Named(name.to_string()))
            }
        } else {
            let start = self.pos();
            let c = self.char();
            if c == '\\' {
                return Err(self.error(self.span_char(), ast::ErrorKind::UnicodeClassInvalid));
            }
            self.bump_and_bump_space();
            let kind = ast::ClassUnicodeKind::OneLetter(c);
            (start, kind)
        };
        Ok(ast::ClassUnicode {
            span: Span::new(start, self.pos()),
            negated,
            kind,
        })
    }
    #[inline(never)]
    fn parse_perl_class(&self) -> ast::ClassPerl {
        let c = self.char();
        let span = self.span_char();
        self.bump();
        let (negated, kind) = match c {
            'd' => (false, ast::ClassPerlKind::Digit),
            'D' => (true, ast::ClassPerlKind::Digit),
            's' => (false, ast::ClassPerlKind::Space),
            'S' => (true, ast::ClassPerlKind::Space),
            'w' => (false, ast::ClassPerlKind::Word),
            'W' => (true, ast::ClassPerlKind::Word),
            c => panic!("expected valid Perl class but got '{c}'"),
        };
        ast::ClassPerl {
            span,
            kind,
            negated,
        }
    }
}
#[derive(Debug)]
struct NestLimiter<'p, 's, P> {
    p: &'p ParserI<'s, P>,
    depth: u32,
}
impl<'p, 's, P: Borrow<Parser>> NestLimiter<'p, 's, P> {
    fn new(p: &'p ParserI<'s, P>) -> NestLimiter<'p, 's, P> {
        NestLimiter { p, depth: 0 }
    }
    #[inline(never)]
    fn check(self, ast: &Ast) -> Result<()> {
        ast::visit(ast, self)
    }
    fn increment_depth(&mut self, span: &Span) -> Result<()> {
        let new = self.depth.checked_add(1).ok_or_else(|| {
            self.p
                .error(span.clone(), ast::ErrorKind::NestLimitExceeded(u32::MAX))
        })?;
        let limit = self.p.parser().nest_limit;
        if new > limit {
            return Err(self
                .p
                .error(span.clone(), ast::ErrorKind::NestLimitExceeded(limit)));
        }
        self.depth = new;
        Ok(())
    }
    fn decrement_depth(&mut self) {
        self.depth = self.depth.checked_sub(1).unwrap();
    }
}
impl<'p, 's, P: Borrow<Parser>> ast::Visitor for NestLimiter<'p, 's, P> {
    type Output = ();
    type Err = ast::Error;
    fn finish(self) -> Result<()> {
        Ok(())
    }
    fn visit_pre(&mut self, ast: &Ast) -> Result<()> {
        let span = match *ast {
            Ast::Empty(_)
            | Ast::Flags(_)
            | Ast::Literal(_)
            | Ast::Dot(_)
            | Ast::Assertion(_)
            | Ast::ClassUnicode(_)
            | Ast::ClassPerl(_) => {
                return Ok(());
            }
            Ast::ClassBracketed(ref x) => &x.span,
            Ast::Repetition(ref x) => &x.span,
            Ast::Group(ref x) => &x.span,
            Ast::Alternation(ref x) => &x.span,
            Ast::Concat(ref x) => &x.span,
        };
        self.increment_depth(span)
    }
    fn visit_post(&mut self, ast: &Ast) -> Result<()> {
        match *ast {
            Ast::Empty(_)
            | Ast::Flags(_)
            | Ast::Literal(_)
            | Ast::Dot(_)
            | Ast::Assertion(_)
            | Ast::ClassUnicode(_)
            | Ast::ClassPerl(_) => Ok(()),
            Ast::ClassBracketed(_)
            | Ast::Repetition(_)
            | Ast::Group(_)
            | Ast::Alternation(_)
            | Ast::Concat(_) => {
                self.decrement_depth();
                Ok(())
            }
        }
    }
    fn visit_class_set_item_pre(&mut self, ast: &ast::ClassSetItem) -> Result<()> {
        let span = match *ast {
            ast::ClassSetItem::Empty(_)
            | ast::ClassSetItem::Literal(_)
            | ast::ClassSetItem::Range(_)
            | ast::ClassSetItem::Ascii(_)
            | ast::ClassSetItem::Unicode(_)
            | ast::ClassSetItem::Perl(_) => {
                return Ok(());
            }
            ast::ClassSetItem::Bracketed(ref x) => &x.span,
            ast::ClassSetItem::Union(ref x) => &x.span,
        };
        self.increment_depth(span)
    }
    fn visit_class_set_item_post(&mut self, ast: &ast::ClassSetItem) -> Result<()> {
        match *ast {
            ast::ClassSetItem::Empty(_)
            | ast::ClassSetItem::Literal(_)
            | ast::ClassSetItem::Range(_)
            | ast::ClassSetItem::Ascii(_)
            | ast::ClassSetItem::Unicode(_)
            | ast::ClassSetItem::Perl(_) => Ok(()),
            ast::ClassSetItem::Bracketed(_) | ast::ClassSetItem::Union(_) => {
                self.decrement_depth();
                Ok(())
            }
        }
    }
    fn visit_class_set_binary_op_pre(&mut self, ast: &ast::ClassSetBinaryOp) -> Result<()> {
        self.increment_depth(&ast.span)
    }
    fn visit_class_set_binary_op_post(&mut self, _ast: &ast::ClassSetBinaryOp) -> Result<()> {
        self.decrement_depth();
        Ok(())
    }
}
fn specialize_err<T>(result: Result<T>, from: ast::ErrorKind, to: ast::ErrorKind) -> Result<T> {
    if let Err(e) = result {
        if e.kind == from {
            Err(ast::Error {
                kind: to,
                pattern: e.pattern,
                span: e.span,
            })
        } else {
            Err(e)
        }
    } else {
        result
    }
}
