use crate::regex_engine::syntax::hir::{self, Hir, HirKind};
use std::{vec, vec::Vec};
pub trait Visitor {
    type Output;
    type Err;
    fn finish(self) -> Result<Self::Output, Self::Err>;
    fn start(&mut self) {}
    fn visit_pre(&mut self, _hir: &Hir) -> Result<(), Self::Err> {
        Ok(())
    }
    fn visit_post(&mut self, _hir: &Hir) -> Result<(), Self::Err> {
        Ok(())
    }
    fn visit_alternation_in(&mut self) -> Result<(), Self::Err> {
        Ok(())
    }
    fn visit_concat_in(&mut self) -> Result<(), Self::Err> {
        Ok(())
    }
}
pub fn visit<V: Visitor>(hir: &Hir, visitor: V) -> Result<V::Output, V::Err> {
    HeapVisitor::new().visit(hir, visitor)
}
struct HeapVisitor<'a> {
    stack: Vec<(&'a Hir, Frame<'a>)>,
}
enum Frame<'a> {
    Repetition(&'a hir::Repetition),
    Capture(&'a hir::Capture),
    Concat { head: &'a Hir, tail: &'a [Hir] },
    Alternation { head: &'a Hir, tail: &'a [Hir] },
}
impl<'a> HeapVisitor<'a> {
    fn new() -> HeapVisitor<'a> {
        HeapVisitor { stack: vec![] }
    }
    fn visit<V: Visitor>(&mut self, mut hir: &'a Hir, mut visitor: V) -> Result<V::Output, V::Err> {
        self.stack.clear();
        visitor.start();
        loop {
            visitor.visit_pre(hir)?;
            if let Some(x) = self.induct(hir) {
                let child = x.child();
                self.stack.push((hir, x));
                hir = child;
                continue;
            }
            visitor.visit_post(hir)?;
            loop {
                let (post_hir, frame) = match self.stack.pop() {
                    None => return visitor.finish(),
                    Some((post_hir, frame)) => (post_hir, frame),
                };
                if let Some(x) = self.pop(frame) {
                    match x {
                        Frame::Alternation { .. } => {
                            visitor.visit_alternation_in()?;
                        }
                        Frame::Concat { .. } => {
                            visitor.visit_concat_in()?;
                        }
                        _ => {}
                    }
                    hir = x.child();
                    self.stack.push((post_hir, x));
                    break;
                }
                visitor.visit_post(post_hir)?;
            }
        }
    }
    fn induct(&mut self, hir: &'a Hir) -> Option<Frame<'a>> {
        match *hir.kind() {
            HirKind::Repetition(ref x) => Some(Frame::Repetition(x)),
            HirKind::Capture(ref x) => Some(Frame::Capture(x)),
            HirKind::Concat(ref x) if x.is_empty() => None,
            HirKind::Concat(ref x) => Some(Frame::Concat {
                head: &x[0],
                tail: &x[1..],
            }),
            HirKind::Alternation(ref x) if x.is_empty() => None,
            HirKind::Alternation(ref x) => Some(Frame::Alternation {
                head: &x[0],
                tail: &x[1..],
            }),
            _ => None,
        }
    }
    fn pop(&self, induct: Frame<'a>) -> Option<Frame<'a>> {
        match induct {
            Frame::Repetition(_) => None,
            Frame::Capture(_) => None,
            Frame::Concat { tail, .. } => {
                if tail.is_empty() {
                    None
                } else {
                    Some(Frame::Concat {
                        head: &tail[0],
                        tail: &tail[1..],
                    })
                }
            }
            Frame::Alternation { tail, .. } => {
                if tail.is_empty() {
                    None
                } else {
                    Some(Frame::Alternation {
                        head: &tail[0],
                        tail: &tail[1..],
                    })
                }
            }
        }
    }
}
impl<'a> Frame<'a> {
    fn child(&self) -> &'a Hir {
        match *self {
            Frame::Repetition(rep) => &rep.sub,
            Frame::Capture(capture) => &capture.sub,
            Frame::Concat { head, .. } => head,
            Frame::Alternation { head, .. } => head,
        }
    }
}
