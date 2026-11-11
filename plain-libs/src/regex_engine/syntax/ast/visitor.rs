use crate::regex_engine::syntax::ast::{self, Ast};
use std::{vec, vec::Vec};
pub trait Visitor {
    type Output;
    type Err;
    fn finish(self) -> Result<Self::Output, Self::Err>;
    fn start(&mut self) {}
    fn visit_pre(&mut self, _ast: &Ast) -> Result<(), Self::Err> {
        Ok(())
    }
    fn visit_post(&mut self, _ast: &Ast) -> Result<(), Self::Err> {
        Ok(())
    }
    fn visit_alternation_in(&mut self) -> Result<(), Self::Err> {
        Ok(())
    }
    fn visit_concat_in(&mut self) -> Result<(), Self::Err> {
        Ok(())
    }
    fn visit_class_set_item_pre(&mut self, _ast: &ast::ClassSetItem) -> Result<(), Self::Err> {
        Ok(())
    }
    fn visit_class_set_item_post(&mut self, _ast: &ast::ClassSetItem) -> Result<(), Self::Err> {
        Ok(())
    }
    fn visit_class_set_binary_op_pre(
        &mut self,
        _ast: &ast::ClassSetBinaryOp,
    ) -> Result<(), Self::Err> {
        Ok(())
    }
    fn visit_class_set_binary_op_post(
        &mut self,
        _ast: &ast::ClassSetBinaryOp,
    ) -> Result<(), Self::Err> {
        Ok(())
    }
    fn visit_class_set_binary_op_in(
        &mut self,
        _ast: &ast::ClassSetBinaryOp,
    ) -> Result<(), Self::Err> {
        Ok(())
    }
}
pub fn visit<V: Visitor>(ast: &Ast, visitor: V) -> Result<V::Output, V::Err> {
    HeapVisitor::new().visit(ast, visitor)
}
struct HeapVisitor<'a> {
    stack: Vec<(&'a Ast, Frame<'a>)>,
    stack_class: Vec<(ClassInduct<'a>, ClassFrame<'a>)>,
}
enum Frame<'a> {
    Repetition(&'a ast::Repetition),
    Group(&'a ast::Group),
    Concat { head: &'a Ast, tail: &'a [Ast] },
    Alternation { head: &'a Ast, tail: &'a [Ast] },
}
enum ClassFrame<'a> {
    Union {
        head: &'a ast::ClassSetItem,
        tail: &'a [ast::ClassSetItem],
    },
    Binary {
        op: &'a ast::ClassSetBinaryOp,
    },
    BinaryLHS {
        op: &'a ast::ClassSetBinaryOp,
        lhs: &'a ast::ClassSet,
        rhs: &'a ast::ClassSet,
    },
    BinaryRHS {
        op: &'a ast::ClassSetBinaryOp,
        rhs: &'a ast::ClassSet,
    },
}
enum ClassInduct<'a> {
    Item(&'a ast::ClassSetItem),
    BinaryOp(&'a ast::ClassSetBinaryOp),
}
impl<'a> HeapVisitor<'a> {
    fn new() -> HeapVisitor<'a> {
        HeapVisitor {
            stack: vec![],
            stack_class: vec![],
        }
    }
    fn visit<V: Visitor>(&mut self, mut ast: &'a Ast, mut visitor: V) -> Result<V::Output, V::Err> {
        self.stack.clear();
        self.stack_class.clear();
        visitor.start();
        loop {
            visitor.visit_pre(ast)?;
            if let Some(x) = self.induct(ast, &mut visitor)? {
                let child = x.child();
                self.stack.push((ast, x));
                ast = child;
                continue;
            }
            visitor.visit_post(ast)?;
            loop {
                let (post_ast, frame) = match self.stack.pop() {
                    None => return visitor.finish(),
                    Some((post_ast, frame)) => (post_ast, frame),
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
                    ast = x.child();
                    self.stack.push((post_ast, x));
                    break;
                }
                visitor.visit_post(post_ast)?;
            }
        }
    }
    fn induct<V: Visitor>(
        &mut self,
        ast: &'a Ast,
        visitor: &mut V,
    ) -> Result<Option<Frame<'a>>, V::Err> {
        Ok(match *ast {
            Ast::ClassBracketed(ref x) => {
                self.visit_class(x, visitor)?;
                None
            }
            Ast::Repetition(ref x) => Some(Frame::Repetition(x)),
            Ast::Group(ref x) => Some(Frame::Group(x)),
            Ast::Concat(ref x) if x.asts.is_empty() => None,
            Ast::Concat(ref x) => Some(Frame::Concat {
                head: &x.asts[0],
                tail: &x.asts[1..],
            }),
            Ast::Alternation(ref x) if x.asts.is_empty() => None,
            Ast::Alternation(ref x) => Some(Frame::Alternation {
                head: &x.asts[0],
                tail: &x.asts[1..],
            }),
            _ => None,
        })
    }
    fn pop(&self, induct: Frame<'a>) -> Option<Frame<'a>> {
        match induct {
            Frame::Repetition(_) => None,
            Frame::Group(_) => None,
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
    fn visit_class<V: Visitor>(
        &mut self,
        ast: &'a ast::ClassBracketed,
        visitor: &mut V,
    ) -> Result<(), V::Err> {
        let mut ast = ClassInduct::from_bracketed(ast);
        loop {
            self.visit_class_pre(&ast, visitor)?;
            if let Some(x) = self.induct_class(&ast) {
                let child = x.child();
                self.stack_class.push((ast, x));
                ast = child;
                continue;
            }
            self.visit_class_post(&ast, visitor)?;
            loop {
                let (post_ast, frame) = match self.stack_class.pop() {
                    None => return Ok(()),
                    Some((post_ast, frame)) => (post_ast, frame),
                };
                if let Some(x) = self.pop_class(frame) {
                    if let ClassFrame::BinaryRHS { ref op, .. } = x {
                        visitor.visit_class_set_binary_op_in(op)?;
                    }
                    ast = x.child();
                    self.stack_class.push((post_ast, x));
                    break;
                }
                self.visit_class_post(&post_ast, visitor)?;
            }
        }
    }
    fn visit_class_pre<V: Visitor>(
        &self,
        ast: &ClassInduct<'a>,
        visitor: &mut V,
    ) -> Result<(), V::Err> {
        match *ast {
            ClassInduct::Item(item) => {
                visitor.visit_class_set_item_pre(item)?;
            }
            ClassInduct::BinaryOp(op) => {
                visitor.visit_class_set_binary_op_pre(op)?;
            }
        }
        Ok(())
    }
    fn visit_class_post<V: Visitor>(
        &self,
        ast: &ClassInduct<'a>,
        visitor: &mut V,
    ) -> Result<(), V::Err> {
        match *ast {
            ClassInduct::Item(item) => {
                visitor.visit_class_set_item_post(item)?;
            }
            ClassInduct::BinaryOp(op) => {
                visitor.visit_class_set_binary_op_post(op)?;
            }
        }
        Ok(())
    }
    fn induct_class(&self, ast: &ClassInduct<'a>) -> Option<ClassFrame<'a>> {
        match *ast {
            ClassInduct::Item(&ast::ClassSetItem::Bracketed(ref x)) => match x.kind {
                ast::ClassSet::Item(ref item) => Some(ClassFrame::Union {
                    head: item,
                    tail: &[],
                }),
                ast::ClassSet::BinaryOp(ref op) => Some(ClassFrame::Binary { op }),
            },
            ClassInduct::Item(&ast::ClassSetItem::Union(ref x)) => {
                if x.items.is_empty() {
                    None
                } else {
                    Some(ClassFrame::Union {
                        head: &x.items[0],
                        tail: &x.items[1..],
                    })
                }
            }
            ClassInduct::BinaryOp(op) => Some(ClassFrame::BinaryLHS {
                op,
                lhs: &op.lhs,
                rhs: &op.rhs,
            }),
            _ => None,
        }
    }
    fn pop_class(&self, induct: ClassFrame<'a>) -> Option<ClassFrame<'a>> {
        match induct {
            ClassFrame::Union { tail, .. } => {
                if tail.is_empty() {
                    None
                } else {
                    Some(ClassFrame::Union {
                        head: &tail[0],
                        tail: &tail[1..],
                    })
                }
            }
            ClassFrame::Binary { .. } => None,
            ClassFrame::BinaryLHS { op, rhs, .. } => Some(ClassFrame::BinaryRHS { op, rhs }),
            ClassFrame::BinaryRHS { .. } => None,
        }
    }
}
impl<'a> Frame<'a> {
    fn child(&self) -> &'a Ast {
        match *self {
            Frame::Repetition(rep) => &rep.ast,
            Frame::Group(group) => &group.ast,
            Frame::Concat { head, .. } => head,
            Frame::Alternation { head, .. } => head,
        }
    }
}
impl<'a> ClassFrame<'a> {
    fn child(&self) -> ClassInduct<'a> {
        match *self {
            ClassFrame::Union { head, .. } => ClassInduct::Item(head),
            ClassFrame::Binary { op, .. } => ClassInduct::BinaryOp(op),
            ClassFrame::BinaryLHS { ref lhs, .. } => ClassInduct::from_set(lhs),
            ClassFrame::BinaryRHS { ref rhs, .. } => ClassInduct::from_set(rhs),
        }
    }
}
impl<'a> ClassInduct<'a> {
    fn from_bracketed(ast: &'a ast::ClassBracketed) -> ClassInduct<'a> {
        ClassInduct::from_set(&ast.kind)
    }
    fn from_set(ast: &'a ast::ClassSet) -> ClassInduct<'a> {
        match *ast {
            ast::ClassSet::Item(ref item) => ClassInduct::Item(item),
            ast::ClassSet::BinaryOp(ref op) => ClassInduct::BinaryOp(op),
        }
    }
}
impl<'a> core::fmt::Debug for ClassFrame<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let x = match *self {
            ClassFrame::Union { .. } => "Union",
            ClassFrame::Binary { .. } => "Binary",
            ClassFrame::BinaryLHS { .. } => "BinaryLHS",
            ClassFrame::BinaryRHS { .. } => "BinaryRHS",
        };
        write!(f, "{x}")
    }
}
impl<'a> core::fmt::Debug for ClassInduct<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let x = match *self {
            ClassInduct::Item(it) => match *it {
                ast::ClassSetItem::Empty(_) => "Item(Empty)",
                ast::ClassSetItem::Literal(_) => "Item(Literal)",
                ast::ClassSetItem::Range(_) => "Item(Range)",
                ast::ClassSetItem::Ascii(_) => "Item(Ascii)",
                ast::ClassSetItem::Perl(_) => "Item(Perl)",
                ast::ClassSetItem::Unicode(_) => "Item(Unicode)",
                ast::ClassSetItem::Bracketed(_) => "Item(Bracketed)",
                ast::ClassSetItem::Union(_) => "Item(Union)",
            },
            ClassInduct::BinaryOp(it) => match it.kind {
                ast::ClassSetBinaryOpKind::Intersection => "BinaryOp(Intersection)",
                ast::ClassSetBinaryOpKind::Difference => "BinaryOp(Difference)",
                ast::ClassSetBinaryOpKind::SymmetricDifference => "BinaryOp(SymmetricDifference)",
            },
        };
        write!(f, "{x}")
    }
}
