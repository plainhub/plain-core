use crate::regex_engine::syntax::{
    ast::{self, Ast, Span, Visitor},
    either::Either,
    hir::{self, Error, ErrorKind, Hir, HirKind},
    unicode::{self, ClassQuery},
};
use core::cell::{Cell, RefCell};
use std::{boxed::Box, string::ToString, vec, vec::Vec};
type Result<T> = core::result::Result<T, Error>;
#[derive(Clone, Debug)]
pub struct TranslatorBuilder {
    utf8: bool,
    line_terminator: u8,
    flags: Flags,
}
impl Default for TranslatorBuilder {
    fn default() -> TranslatorBuilder {
        TranslatorBuilder::new()
    }
}
impl TranslatorBuilder {
    pub fn new() -> TranslatorBuilder {
        TranslatorBuilder {
            utf8: true,
            line_terminator: b'\n',
            flags: Flags::default(),
        }
    }
    pub fn build(&self) -> Translator {
        Translator {
            stack: RefCell::new(vec![]),
            flags: Cell::new(self.flags),
            utf8: self.utf8,
            line_terminator: self.line_terminator,
        }
    }
    pub fn utf8(&mut self, yes: bool) -> &mut TranslatorBuilder {
        self.utf8 = yes;
        self
    }
    pub fn line_terminator(&mut self, byte: u8) -> &mut TranslatorBuilder {
        self.line_terminator = byte;
        self
    }
    pub fn case_insensitive(&mut self, yes: bool) -> &mut TranslatorBuilder {
        self.flags.case_insensitive = if yes { Some(true) } else { None };
        self
    }
    pub fn multi_line(&mut self, yes: bool) -> &mut TranslatorBuilder {
        self.flags.multi_line = if yes { Some(true) } else { None };
        self
    }
    pub fn dot_matches_new_line(&mut self, yes: bool) -> &mut TranslatorBuilder {
        self.flags.dot_matches_new_line = if yes { Some(true) } else { None };
        self
    }
    pub fn crlf(&mut self, yes: bool) -> &mut TranslatorBuilder {
        self.flags.crlf = if yes { Some(true) } else { None };
        self
    }
    pub fn swap_greed(&mut self, yes: bool) -> &mut TranslatorBuilder {
        self.flags.swap_greed = if yes { Some(true) } else { None };
        self
    }
    pub fn unicode(&mut self, yes: bool) -> &mut TranslatorBuilder {
        self.flags.unicode = if yes { None } else { Some(false) };
        self
    }
}
#[derive(Clone, Debug)]
pub struct Translator {
    stack: RefCell<Vec<HirFrame>>,
    flags: Cell<Flags>,
    utf8: bool,
    line_terminator: u8,
}
impl Translator {
    pub fn new() -> Translator {
        TranslatorBuilder::new().build()
    }
    pub fn translate(&mut self, pattern: &str, ast: &Ast) -> Result<Hir> {
        ast::visit(ast, TranslatorI::new(self, pattern))
    }
}
#[derive(Clone, Debug)]
enum HirFrame {
    Expr(Hir),
    Literal(Vec<u8>),
    ClassUnicode(hir::ClassUnicode),
    ClassBytes(hir::ClassBytes),
    Repetition,
    Group { old_flags: Flags },
    Concat,
    Alternation,
    AlternationBranch,
}
impl HirFrame {
    fn unwrap_expr(self) -> Hir {
        match self {
            HirFrame::Expr(expr) => expr,
            HirFrame::Literal(lit) => Hir::literal(lit),
            _ => panic!("tried to unwrap expr from HirFrame, got: {self:?}"),
        }
    }
    fn unwrap_class_unicode(self) -> hir::ClassUnicode {
        match self {
            HirFrame::ClassUnicode(cls) => cls,
            _ => panic!(
                "tried to unwrap Unicode class \
                 from HirFrame, got: {:?}",
                self
            ),
        }
    }
    fn unwrap_class_bytes(self) -> hir::ClassBytes {
        match self {
            HirFrame::ClassBytes(cls) => cls,
            _ => panic!(
                "tried to unwrap byte class \
                 from HirFrame, got: {:?}",
                self
            ),
        }
    }
    fn unwrap_repetition(self) {
        match self {
            HirFrame::Repetition => {}
            _ => {
                panic!("tried to unwrap repetition from HirFrame, got: {self:?}")
            }
        }
    }
    fn unwrap_group(self) -> Flags {
        match self {
            HirFrame::Group { old_flags } => old_flags,
            _ => {
                panic!("tried to unwrap group from HirFrame, got: {self:?}")
            }
        }
    }
    fn unwrap_alternation_pipe(self) {
        match self {
            HirFrame::AlternationBranch => {}
            _ => {
                panic!("tried to unwrap alt pipe from HirFrame, got: {self:?}")
            }
        }
    }
}
impl<'t, 'p> Visitor for TranslatorI<'t, 'p> {
    type Output = Hir;
    type Err = Error;
    fn finish(self) -> Result<Hir> {
        assert_eq!(self.trans().stack.borrow().len(), 1);
        Ok(self.pop().unwrap().unwrap_expr())
    }
    fn visit_pre(&mut self, ast: &Ast) -> Result<()> {
        match *ast {
            Ast::ClassBracketed(_) => {
                if self.flags().unicode() {
                    let cls = hir::ClassUnicode::empty();
                    self.push(HirFrame::ClassUnicode(cls));
                } else {
                    let cls = hir::ClassBytes::empty();
                    self.push(HirFrame::ClassBytes(cls));
                }
            }
            Ast::Repetition(_) => self.push(HirFrame::Repetition),
            Ast::Group(ref x) => {
                let old_flags = x
                    .flags()
                    .map(|ast| self.set_flags(ast))
                    .unwrap_or_else(|| self.flags());
                self.push(HirFrame::Group { old_flags });
            }
            Ast::Concat(_) => {
                self.push(HirFrame::Concat);
            }
            Ast::Alternation(ref x) => {
                self.push(HirFrame::Alternation);
                if !x.asts.is_empty() {
                    self.push(HirFrame::AlternationBranch);
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn visit_post(&mut self, ast: &Ast) -> Result<()> {
        match *ast {
            Ast::Empty(_) => {
                self.push(HirFrame::Expr(Hir::empty()));
            }
            Ast::Flags(ref x) => {
                self.set_flags(&x.flags);
                self.push(HirFrame::Expr(Hir::empty()));
            }
            Ast::Literal(ref x) => match self.ast_literal_to_scalar(x)? {
                Either::Right(byte) => self.push_byte(byte),
                Either::Left(ch) => match self.case_fold_char(x.span, ch)? {
                    None => self.push_char(ch),
                    Some(expr) => self.push(HirFrame::Expr(expr)),
                },
            },
            Ast::Dot(ref span) => {
                self.push(HirFrame::Expr(self.hir_dot(**span)?));
            }
            Ast::Assertion(ref x) => {
                self.push(HirFrame::Expr(self.hir_assertion(x)?));
            }
            Ast::ClassPerl(ref x) => {
                if self.flags().unicode() {
                    let cls = self.hir_perl_unicode_class(x)?;
                    let hcls = hir::Class::Unicode(cls);
                    self.push(HirFrame::Expr(Hir::class(hcls)));
                } else {
                    let cls = self.hir_perl_byte_class(x)?;
                    let hcls = hir::Class::Bytes(cls);
                    self.push(HirFrame::Expr(Hir::class(hcls)));
                }
            }
            Ast::ClassUnicode(ref x) => {
                let cls = hir::Class::Unicode(self.hir_unicode_class(x)?);
                self.push(HirFrame::Expr(Hir::class(cls)));
            }
            Ast::ClassBracketed(ref ast) => {
                if self.flags().unicode() {
                    let mut cls = self.pop().unwrap().unwrap_class_unicode();
                    self.unicode_fold_and_negate(&ast.span, ast.negated, &mut cls)?;
                    let expr = Hir::class(hir::Class::Unicode(cls));
                    self.push(HirFrame::Expr(expr));
                } else {
                    let mut cls = self.pop().unwrap().unwrap_class_bytes();
                    self.bytes_fold_and_negate(&ast.span, ast.negated, &mut cls)?;
                    let expr = Hir::class(hir::Class::Bytes(cls));
                    self.push(HirFrame::Expr(expr));
                }
            }
            Ast::Repetition(ref x) => {
                let expr = self.pop().unwrap().unwrap_expr();
                self.pop().unwrap().unwrap_repetition();
                self.push(HirFrame::Expr(self.hir_repetition(x, expr)));
            }
            Ast::Group(ref x) => {
                let expr = self.pop().unwrap().unwrap_expr();
                let old_flags = self.pop().unwrap().unwrap_group();
                self.trans().flags.set(old_flags);
                self.push(HirFrame::Expr(self.hir_capture(x, expr)));
            }
            Ast::Concat(_) => {
                let mut exprs = vec![];
                while let Some(expr) = self.pop_concat_expr() {
                    if !matches!(*expr.kind(), HirKind::Empty) {
                        exprs.push(expr);
                    }
                }
                exprs.reverse();
                self.push(HirFrame::Expr(Hir::concat(exprs)));
            }
            Ast::Alternation(_) => {
                let mut exprs = vec![];
                while let Some(expr) = self.pop_alt_expr() {
                    self.pop().unwrap().unwrap_alternation_pipe();
                    exprs.push(expr);
                }
                exprs.reverse();
                self.push(HirFrame::Expr(Hir::alternation(exprs)));
            }
        }
        Ok(())
    }
    fn visit_alternation_in(&mut self) -> Result<()> {
        self.push(HirFrame::AlternationBranch);
        Ok(())
    }
    fn visit_class_set_item_pre(&mut self, ast: &ast::ClassSetItem) -> Result<()> {
        match *ast {
            ast::ClassSetItem::Bracketed(_) => {
                if self.flags().unicode() {
                    let cls = hir::ClassUnicode::empty();
                    self.push(HirFrame::ClassUnicode(cls));
                } else {
                    let cls = hir::ClassBytes::empty();
                    self.push(HirFrame::ClassBytes(cls));
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn visit_class_set_item_post(&mut self, ast: &ast::ClassSetItem) -> Result<()> {
        match *ast {
            ast::ClassSetItem::Empty(_) => {}
            ast::ClassSetItem::Literal(ref x) => {
                if self.flags().unicode() {
                    let mut cls = self.pop().unwrap().unwrap_class_unicode();
                    cls.push(hir::ClassUnicodeRange::new(x.c, x.c));
                    self.push(HirFrame::ClassUnicode(cls));
                } else {
                    let mut cls = self.pop().unwrap().unwrap_class_bytes();
                    let byte = self.class_literal_byte(x)?;
                    cls.push(hir::ClassBytesRange::new(byte, byte));
                    self.push(HirFrame::ClassBytes(cls));
                }
            }
            ast::ClassSetItem::Range(ref x) => {
                if self.flags().unicode() {
                    let mut cls = self.pop().unwrap().unwrap_class_unicode();
                    cls.push(hir::ClassUnicodeRange::new(x.start.c, x.end.c));
                    self.push(HirFrame::ClassUnicode(cls));
                } else {
                    let mut cls = self.pop().unwrap().unwrap_class_bytes();
                    let start = self.class_literal_byte(&x.start)?;
                    let end = self.class_literal_byte(&x.end)?;
                    cls.push(hir::ClassBytesRange::new(start, end));
                    self.push(HirFrame::ClassBytes(cls));
                }
            }
            ast::ClassSetItem::Ascii(ref x) => {
                if self.flags().unicode() {
                    let xcls = self.hir_ascii_unicode_class(x)?;
                    let mut cls = self.pop().unwrap().unwrap_class_unicode();
                    cls.union(&xcls);
                    self.push(HirFrame::ClassUnicode(cls));
                } else {
                    let xcls = self.hir_ascii_byte_class(x)?;
                    let mut cls = self.pop().unwrap().unwrap_class_bytes();
                    cls.union(&xcls);
                    self.push(HirFrame::ClassBytes(cls));
                }
            }
            ast::ClassSetItem::Unicode(ref x) => {
                let xcls = self.hir_unicode_class(x)?;
                let mut cls = self.pop().unwrap().unwrap_class_unicode();
                cls.union(&xcls);
                self.push(HirFrame::ClassUnicode(cls));
            }
            ast::ClassSetItem::Perl(ref x) => {
                if self.flags().unicode() {
                    let xcls = self.hir_perl_unicode_class(x)?;
                    let mut cls = self.pop().unwrap().unwrap_class_unicode();
                    cls.union(&xcls);
                    self.push(HirFrame::ClassUnicode(cls));
                } else {
                    let xcls = self.hir_perl_byte_class(x)?;
                    let mut cls = self.pop().unwrap().unwrap_class_bytes();
                    cls.union(&xcls);
                    self.push(HirFrame::ClassBytes(cls));
                }
            }
            ast::ClassSetItem::Bracketed(ref ast) => {
                if self.flags().unicode() {
                    let mut cls1 = self.pop().unwrap().unwrap_class_unicode();
                    self.unicode_fold_and_negate(&ast.span, ast.negated, &mut cls1)?;
                    let mut cls2 = self.pop().unwrap().unwrap_class_unicode();
                    cls2.union(&cls1);
                    self.push(HirFrame::ClassUnicode(cls2));
                } else {
                    let mut cls1 = self.pop().unwrap().unwrap_class_bytes();
                    self.bytes_fold_and_negate(&ast.span, ast.negated, &mut cls1)?;
                    let mut cls2 = self.pop().unwrap().unwrap_class_bytes();
                    cls2.union(&cls1);
                    self.push(HirFrame::ClassBytes(cls2));
                }
            }
            ast::ClassSetItem::Union(_) => {}
        }
        Ok(())
    }
    fn visit_class_set_binary_op_pre(&mut self, _op: &ast::ClassSetBinaryOp) -> Result<()> {
        if self.flags().unicode() {
            let cls = hir::ClassUnicode::empty();
            self.push(HirFrame::ClassUnicode(cls));
        } else {
            let cls = hir::ClassBytes::empty();
            self.push(HirFrame::ClassBytes(cls));
        }
        Ok(())
    }
    fn visit_class_set_binary_op_in(&mut self, _op: &ast::ClassSetBinaryOp) -> Result<()> {
        if self.flags().unicode() {
            let cls = hir::ClassUnicode::empty();
            self.push(HirFrame::ClassUnicode(cls));
        } else {
            let cls = hir::ClassBytes::empty();
            self.push(HirFrame::ClassBytes(cls));
        }
        Ok(())
    }
    fn visit_class_set_binary_op_post(&mut self, op: &ast::ClassSetBinaryOp) -> Result<()> {
        use crate::regex_engine::syntax::ast::ClassSetBinaryOpKind::*;
        if self.flags().unicode() {
            let mut rhs = self.pop().unwrap().unwrap_class_unicode();
            let mut lhs = self.pop().unwrap().unwrap_class_unicode();
            let mut cls = self.pop().unwrap().unwrap_class_unicode();
            if self.flags().case_insensitive() {
                rhs.try_case_fold_simple().map_err(|_| {
                    self.error(op.rhs.span().clone(), ErrorKind::UnicodeCaseUnavailable)
                })?;
                lhs.try_case_fold_simple().map_err(|_| {
                    self.error(op.lhs.span().clone(), ErrorKind::UnicodeCaseUnavailable)
                })?;
            }
            match op.kind {
                Intersection => lhs.intersect(&rhs),
                Difference => lhs.difference(&rhs),
                SymmetricDifference => lhs.symmetric_difference(&rhs),
            }
            cls.union(&lhs);
            self.push(HirFrame::ClassUnicode(cls));
        } else {
            let mut rhs = self.pop().unwrap().unwrap_class_bytes();
            let mut lhs = self.pop().unwrap().unwrap_class_bytes();
            let mut cls = self.pop().unwrap().unwrap_class_bytes();
            if self.flags().case_insensitive() {
                rhs.case_fold_simple();
                lhs.case_fold_simple();
            }
            match op.kind {
                Intersection => lhs.intersect(&rhs),
                Difference => lhs.difference(&rhs),
                SymmetricDifference => lhs.symmetric_difference(&rhs),
            }
            cls.union(&lhs);
            self.push(HirFrame::ClassBytes(cls));
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
struct TranslatorI<'t, 'p> {
    trans: &'t Translator,
    pattern: &'p str,
}
impl<'t, 'p> TranslatorI<'t, 'p> {
    fn new(trans: &'t Translator, pattern: &'p str) -> TranslatorI<'t, 'p> {
        TranslatorI { trans, pattern }
    }
    fn trans(&self) -> &Translator {
        &self.trans
    }
    fn push(&self, frame: HirFrame) {
        self.trans().stack.borrow_mut().push(frame);
    }
    fn push_char(&self, ch: char) {
        let mut buf = [0; 4];
        let bytes = ch.encode_utf8(&mut buf).as_bytes();
        let mut stack = self.trans().stack.borrow_mut();
        if let Some(HirFrame::Literal(literal)) = stack.last_mut() {
            literal.extend_from_slice(bytes);
        } else {
            stack.push(HirFrame::Literal(bytes.to_vec()));
        }
    }
    fn push_byte(&self, byte: u8) {
        let mut stack = self.trans().stack.borrow_mut();
        if let Some(HirFrame::Literal(literal)) = stack.last_mut() {
            literal.push(byte);
        } else {
            stack.push(HirFrame::Literal(vec![byte]));
        }
    }
    fn pop(&self) -> Option<HirFrame> {
        self.trans().stack.borrow_mut().pop()
    }
    fn pop_concat_expr(&self) -> Option<Hir> {
        let frame = self.pop()?;
        match frame {
            HirFrame::Concat => None,
            HirFrame::Expr(expr) => Some(expr),
            HirFrame::Literal(lit) => Some(Hir::literal(lit)),
            HirFrame::ClassUnicode(_) => {
                unreachable!("expected expr or concat, got Unicode class")
            }
            HirFrame::ClassBytes(_) => {
                unreachable!("expected expr or concat, got byte class")
            }
            HirFrame::Repetition => {
                unreachable!("expected expr or concat, got repetition")
            }
            HirFrame::Group { .. } => {
                unreachable!("expected expr or concat, got group")
            }
            HirFrame::Alternation => {
                unreachable!("expected expr or concat, got alt marker")
            }
            HirFrame::AlternationBranch => {
                unreachable!("expected expr or concat, got alt branch marker")
            }
        }
    }
    fn pop_alt_expr(&self) -> Option<Hir> {
        let frame = self.pop()?;
        match frame {
            HirFrame::Alternation => None,
            HirFrame::Expr(expr) => Some(expr),
            HirFrame::Literal(lit) => Some(Hir::literal(lit)),
            HirFrame::ClassUnicode(_) => {
                unreachable!("expected expr or alt, got Unicode class")
            }
            HirFrame::ClassBytes(_) => {
                unreachable!("expected expr or alt, got byte class")
            }
            HirFrame::Repetition => {
                unreachable!("expected expr or alt, got repetition")
            }
            HirFrame::Group { .. } => {
                unreachable!("expected expr or alt, got group")
            }
            HirFrame::Concat => {
                unreachable!("expected expr or alt, got concat marker")
            }
            HirFrame::AlternationBranch => {
                unreachable!("expected expr or alt, got alt branch marker")
            }
        }
    }
    fn error(&self, span: Span, kind: ErrorKind) -> Error {
        Error {
            kind,
            pattern: self.pattern.to_string(),
            span,
        }
    }
    fn flags(&self) -> Flags {
        self.trans().flags.get()
    }
    fn set_flags(&self, ast_flags: &ast::Flags) -> Flags {
        let old_flags = self.flags();
        let mut new_flags = Flags::from_ast(ast_flags);
        new_flags.merge(&old_flags);
        self.trans().flags.set(new_flags);
        old_flags
    }
    fn ast_literal_to_scalar(&self, lit: &ast::Literal) -> Result<Either<char, u8>> {
        if self.flags().unicode() {
            return Ok(Either::Left(lit.c));
        }
        let byte = match lit.byte() {
            None => return Ok(Either::Left(lit.c)),
            Some(byte) => byte,
        };
        if byte <= 0x7F {
            return Ok(Either::Left(char::try_from(byte).unwrap()));
        }
        if self.trans().utf8 {
            return Err(self.error(lit.span, ErrorKind::InvalidUtf8));
        }
        Ok(Either::Right(byte))
    }
    fn case_fold_char(&self, span: Span, c: char) -> Result<Option<Hir>> {
        if !self.flags().case_insensitive() {
            return Ok(None);
        }
        if self.flags().unicode() {
            let map = unicode::SimpleCaseFolder::new()
                .map(|f| f.overlaps(c, c))
                .map_err(|_| self.error(span, ErrorKind::UnicodeCaseUnavailable))?;
            if !map {
                return Ok(None);
            }
            let mut cls = hir::ClassUnicode::new(vec![hir::ClassUnicodeRange::new(c, c)]);
            cls.try_case_fold_simple()
                .map_err(|_| self.error(span, ErrorKind::UnicodeCaseUnavailable))?;
            Ok(Some(Hir::class(hir::Class::Unicode(cls))))
        } else {
            if !c.is_ascii() {
                return Ok(None);
            }
            match c {
                'A'..='Z' | 'a'..='z' => {}
                _ => return Ok(None),
            }
            let mut cls = hir::ClassBytes::new(vec![hir::ClassBytesRange::new(
                u8::try_from(c).unwrap(),
                u8::try_from(c).unwrap(),
            )]);
            cls.case_fold_simple();
            Ok(Some(Hir::class(hir::Class::Bytes(cls))))
        }
    }
    fn hir_dot(&self, span: Span) -> Result<Hir> {
        let (utf8, lineterm, flags) = (
            self.trans().utf8,
            self.trans().line_terminator,
            self.flags(),
        );
        if utf8 && (!flags.unicode() || !lineterm.is_ascii()) {
            return Err(self.error(span, ErrorKind::InvalidUtf8));
        }
        let dot = if flags.dot_matches_new_line() {
            if flags.unicode() {
                hir::Dot::AnyChar
            } else {
                hir::Dot::AnyByte
            }
        } else {
            if flags.unicode() {
                if flags.crlf() {
                    hir::Dot::AnyCharExceptCRLF
                } else {
                    if !lineterm.is_ascii() {
                        return Err(self.error(span, ErrorKind::InvalidLineTerminator));
                    }
                    hir::Dot::AnyCharExcept(char::from(lineterm))
                }
            } else {
                if flags.crlf() {
                    hir::Dot::AnyByteExceptCRLF
                } else {
                    hir::Dot::AnyByteExcept(lineterm)
                }
            }
        };
        Ok(Hir::dot(dot))
    }
    fn hir_assertion(&self, asst: &ast::Assertion) -> Result<Hir> {
        let unicode = self.flags().unicode();
        let multi_line = self.flags().multi_line();
        let crlf = self.flags().crlf();
        Ok(match asst.kind {
            ast::AssertionKind::StartLine => Hir::look(if multi_line {
                if crlf {
                    hir::Look::StartCRLF
                } else {
                    hir::Look::StartLF
                }
            } else {
                hir::Look::Start
            }),
            ast::AssertionKind::EndLine => Hir::look(if multi_line {
                if crlf {
                    hir::Look::EndCRLF
                } else {
                    hir::Look::EndLF
                }
            } else {
                hir::Look::End
            }),
            ast::AssertionKind::StartText => Hir::look(hir::Look::Start),
            ast::AssertionKind::EndText => Hir::look(hir::Look::End),
            ast::AssertionKind::WordBoundary => Hir::look(if unicode {
                hir::Look::WordUnicode
            } else {
                hir::Look::WordAscii
            }),
            ast::AssertionKind::NotWordBoundary => Hir::look(if unicode {
                hir::Look::WordUnicodeNegate
            } else {
                hir::Look::WordAsciiNegate
            }),
            ast::AssertionKind::WordBoundaryStart | ast::AssertionKind::WordBoundaryStartAngle => {
                Hir::look(if unicode {
                    hir::Look::WordStartUnicode
                } else {
                    hir::Look::WordStartAscii
                })
            }
            ast::AssertionKind::WordBoundaryEnd | ast::AssertionKind::WordBoundaryEndAngle => {
                Hir::look(if unicode {
                    hir::Look::WordEndUnicode
                } else {
                    hir::Look::WordEndAscii
                })
            }
            ast::AssertionKind::WordBoundaryStartHalf => Hir::look(if unicode {
                hir::Look::WordStartHalfUnicode
            } else {
                hir::Look::WordStartHalfAscii
            }),
            ast::AssertionKind::WordBoundaryEndHalf => Hir::look(if unicode {
                hir::Look::WordEndHalfUnicode
            } else {
                hir::Look::WordEndHalfAscii
            }),
        })
    }
    fn hir_capture(&self, group: &ast::Group, expr: Hir) -> Hir {
        let (index, name) = match group.kind {
            ast::GroupKind::CaptureIndex(index) => (index, None),
            ast::GroupKind::CaptureName { ref name, .. } => {
                (name.index, Some(name.name.clone().into_boxed_str()))
            }
            ast::GroupKind::NonCapturing(_) => return expr,
        };
        Hir::capture(hir::Capture {
            index,
            name,
            sub: Box::new(expr),
        })
    }
    fn hir_repetition(&self, rep: &ast::Repetition, expr: Hir) -> Hir {
        let (min, max) = match rep.op.kind {
            ast::RepetitionKind::ZeroOrOne => (0, Some(1)),
            ast::RepetitionKind::ZeroOrMore => (0, None),
            ast::RepetitionKind::OneOrMore => (1, None),
            ast::RepetitionKind::Range(ast::RepetitionRange::Exactly(m)) => (m, Some(m)),
            ast::RepetitionKind::Range(ast::RepetitionRange::AtLeast(m)) => (m, None),
            ast::RepetitionKind::Range(ast::RepetitionRange::Bounded(m, n)) => (m, Some(n)),
        };
        let greedy = if self.flags().swap_greed() {
            !rep.greedy
        } else {
            rep.greedy
        };
        Hir::repetition(hir::Repetition {
            min,
            max,
            greedy,
            sub: Box::new(expr),
        })
    }
    fn hir_unicode_class(&self, ast_class: &ast::ClassUnicode) -> Result<hir::ClassUnicode> {
        use crate::regex_engine::syntax::ast::ClassUnicodeKind::*;
        if !self.flags().unicode() {
            return Err(self.error(ast_class.span, ErrorKind::UnicodeNotAllowed));
        }
        let query = match ast_class.kind {
            OneLetter(name) => ClassQuery::OneLetter(name),
            Named(ref name) => ClassQuery::Binary(name),
            NamedValue {
                ref name,
                ref value,
                ..
            } => ClassQuery::ByValue {
                property_name: name,
                property_value: value,
            },
        };
        let mut result = self.convert_unicode_class_error(&ast_class.span, unicode::class(query));
        if let Ok(ref mut class) = result {
            self.unicode_fold_and_negate(&ast_class.span, ast_class.is_negated(), class)?;
        }
        result
    }
    fn hir_ascii_unicode_class(&self, ast: &ast::ClassAscii) -> Result<hir::ClassUnicode> {
        let mut cls = hir::ClassUnicode::new(
            ascii_class_as_chars(&ast.kind).map(|(s, e)| hir::ClassUnicodeRange::new(s, e)),
        );
        self.unicode_fold_and_negate(&ast.span, ast.negated, &mut cls)?;
        Ok(cls)
    }
    fn hir_ascii_byte_class(&self, ast: &ast::ClassAscii) -> Result<hir::ClassBytes> {
        let mut cls = hir::ClassBytes::new(
            ascii_class(&ast.kind).map(|(s, e)| hir::ClassBytesRange::new(s, e)),
        );
        self.bytes_fold_and_negate(&ast.span, ast.negated, &mut cls)?;
        Ok(cls)
    }
    fn hir_perl_unicode_class(&self, ast_class: &ast::ClassPerl) -> Result<hir::ClassUnicode> {
        use crate::regex_engine::syntax::ast::ClassPerlKind::*;
        assert!(self.flags().unicode());
        let result = match ast_class.kind {
            Digit => unicode::perl_digit(),
            Space => unicode::perl_space(),
            Word => unicode::perl_word(),
        };
        let mut class = self.convert_unicode_class_error(&ast_class.span, result)?;
        if ast_class.negated {
            class.negate();
        }
        Ok(class)
    }
    fn hir_perl_byte_class(&self, ast_class: &ast::ClassPerl) -> Result<hir::ClassBytes> {
        use crate::regex_engine::syntax::ast::ClassPerlKind::*;
        assert!(!self.flags().unicode());
        let mut class = match ast_class.kind {
            Digit => hir_ascii_class_bytes(&ast::ClassAsciiKind::Digit),
            Space => hir_ascii_class_bytes(&ast::ClassAsciiKind::Space),
            Word => hir_ascii_class_bytes(&ast::ClassAsciiKind::Word),
        };
        if ast_class.negated {
            class.negate();
        }
        if self.trans().utf8 && !class.is_ascii() {
            return Err(self.error(ast_class.span, ErrorKind::InvalidUtf8));
        }
        Ok(class)
    }
    fn convert_unicode_class_error(
        &self,
        span: &Span,
        result: core::result::Result<hir::ClassUnicode, unicode::Error>,
    ) -> Result<hir::ClassUnicode> {
        result.map_err(|err| {
            let sp = span.clone();
            match err {
                unicode::Error::PropertyNotFound => {
                    self.error(sp, ErrorKind::UnicodePropertyNotFound)
                }
                unicode::Error::PropertyValueNotFound => {
                    self.error(sp, ErrorKind::UnicodePropertyValueNotFound)
                }
                unicode::Error::PerlClassNotFound => {
                    self.error(sp, ErrorKind::UnicodePerlClassNotFound)
                }
            }
        })
    }
    fn unicode_fold_and_negate(
        &self,
        span: &Span,
        negated: bool,
        class: &mut hir::ClassUnicode,
    ) -> Result<()> {
        if self.flags().case_insensitive() {
            class
                .try_case_fold_simple()
                .map_err(|_| self.error(span.clone(), ErrorKind::UnicodeCaseUnavailable))?;
        }
        if negated {
            class.negate();
        }
        Ok(())
    }
    fn bytes_fold_and_negate(
        &self,
        span: &Span,
        negated: bool,
        class: &mut hir::ClassBytes,
    ) -> Result<()> {
        if self.flags().case_insensitive() {
            class.case_fold_simple();
        }
        if negated {
            class.negate();
        }
        if self.trans().utf8 && !class.is_ascii() {
            return Err(self.error(span.clone(), ErrorKind::InvalidUtf8));
        }
        Ok(())
    }
    fn class_literal_byte(&self, ast: &ast::Literal) -> Result<u8> {
        match self.ast_literal_to_scalar(ast)? {
            Either::Right(byte) => Ok(byte),
            Either::Left(ch) => {
                if ch.is_ascii() {
                    Ok(u8::try_from(ch).unwrap())
                } else {
                    Err(self.error(ast.span, ErrorKind::UnicodeNotAllowed))
                }
            }
        }
    }
}
#[derive(Clone, Copy, Debug, Default)]
struct Flags {
    case_insensitive: Option<bool>,
    multi_line: Option<bool>,
    dot_matches_new_line: Option<bool>,
    swap_greed: Option<bool>,
    unicode: Option<bool>,
    crlf: Option<bool>,
}
impl Flags {
    fn from_ast(ast: &ast::Flags) -> Flags {
        let mut flags = Flags::default();
        let mut enable = true;
        for item in &ast.items {
            match item.kind {
                ast::FlagsItemKind::Negation => {
                    enable = false;
                }
                ast::FlagsItemKind::Flag(ast::Flag::CaseInsensitive) => {
                    flags.case_insensitive = Some(enable);
                }
                ast::FlagsItemKind::Flag(ast::Flag::MultiLine) => {
                    flags.multi_line = Some(enable);
                }
                ast::FlagsItemKind::Flag(ast::Flag::DotMatchesNewLine) => {
                    flags.dot_matches_new_line = Some(enable);
                }
                ast::FlagsItemKind::Flag(ast::Flag::SwapGreed) => {
                    flags.swap_greed = Some(enable);
                }
                ast::FlagsItemKind::Flag(ast::Flag::Unicode) => {
                    flags.unicode = Some(enable);
                }
                ast::FlagsItemKind::Flag(ast::Flag::CRLF) => {
                    flags.crlf = Some(enable);
                }
                ast::FlagsItemKind::Flag(ast::Flag::IgnoreWhitespace) => {}
            }
        }
        flags
    }
    fn merge(&mut self, previous: &Flags) {
        if self.case_insensitive.is_none() {
            self.case_insensitive = previous.case_insensitive;
        }
        if self.multi_line.is_none() {
            self.multi_line = previous.multi_line;
        }
        if self.dot_matches_new_line.is_none() {
            self.dot_matches_new_line = previous.dot_matches_new_line;
        }
        if self.swap_greed.is_none() {
            self.swap_greed = previous.swap_greed;
        }
        if self.unicode.is_none() {
            self.unicode = previous.unicode;
        }
        if self.crlf.is_none() {
            self.crlf = previous.crlf;
        }
    }
    fn case_insensitive(&self) -> bool {
        self.case_insensitive.unwrap_or(false)
    }
    fn multi_line(&self) -> bool {
        self.multi_line.unwrap_or(false)
    }
    fn dot_matches_new_line(&self) -> bool {
        self.dot_matches_new_line.unwrap_or(false)
    }
    fn swap_greed(&self) -> bool {
        self.swap_greed.unwrap_or(false)
    }
    fn unicode(&self) -> bool {
        self.unicode.unwrap_or(true)
    }
    fn crlf(&self) -> bool {
        self.crlf.unwrap_or(false)
    }
}
fn hir_ascii_class_bytes(kind: &ast::ClassAsciiKind) -> hir::ClassBytes {
    let ranges: Vec<_> = ascii_class(kind)
        .map(|(s, e)| hir::ClassBytesRange::new(s, e))
        .collect();
    hir::ClassBytes::new(ranges)
}
fn ascii_class(kind: &ast::ClassAsciiKind) -> impl Iterator<Item = (u8, u8)> {
    use crate::regex_engine::syntax::ast::ClassAsciiKind::*;
    let slice: &'static [(u8, u8)] = match *kind {
        Alnum => &[(b'0', b'9'), (b'A', b'Z'), (b'a', b'z')],
        Alpha => &[(b'A', b'Z'), (b'a', b'z')],
        Ascii => &[(b'\x00', b'\x7F')],
        Blank => &[(b'\t', b'\t'), (b' ', b' ')],
        Cntrl => &[(b'\x00', b'\x1F'), (b'\x7F', b'\x7F')],
        Digit => &[(b'0', b'9')],
        Graph => &[(b'!', b'~')],
        Lower => &[(b'a', b'z')],
        Print => &[(b' ', b'~')],
        Punct => &[(b'!', b'/'), (b':', b'@'), (b'[', b'`'), (b'{', b'~')],
        Space => &[
            (b'\t', b'\t'),
            (b'\n', b'\n'),
            (b'\x0B', b'\x0B'),
            (b'\x0C', b'\x0C'),
            (b'\r', b'\r'),
            (b' ', b' '),
        ],
        Upper => &[(b'A', b'Z')],
        Word => &[(b'0', b'9'), (b'A', b'Z'), (b'_', b'_'), (b'a', b'z')],
        Xdigit => &[(b'0', b'9'), (b'A', b'F'), (b'a', b'f')],
    };
    slice.iter().copied()
}
fn ascii_class_as_chars(kind: &ast::ClassAsciiKind) -> impl Iterator<Item = (char, char)> {
    ascii_class(kind).map(|(s, e)| (char::from(s), char::from(e)))
}
