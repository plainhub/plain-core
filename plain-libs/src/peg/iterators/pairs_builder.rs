// pest. The Elegant Parser
// Copyright (c) 2018 Dragoș Tiselice
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use alloc::rc::Rc;
use alloc::vec::Vec;

use super::line_index::LineIndex;
use super::pairs::{self, Pairs};
use super::queueable_token::QueueableToken;
use crate::peg::RuleType;

#[derive(Clone, Debug)]
pub struct PairsBuilder<'i, R> {
    input: &'i str,
    nodes: Vec<BuilderNode<'i, R>>,
}

#[derive(Clone, Debug)]
struct BuilderNode<'i, R> {
    rule: R,
    start: usize,
    end: usize,
    tag: Option<&'i str>,
    children: Vec<BuilderNode<'i, R>>,
}

impl<'i, R: RuleType> PairsBuilder<'i, R> {
    pub fn new(input: &'i str) -> Self {
        PairsBuilder {
            input,
            nodes: Vec::new(),
        }
    }

    pub fn rule(mut self, rule: R, start: usize, end: usize) -> Self {
        self.nodes.push(BuilderNode {
            rule,
            start,
            end,
            tag: None,
            children: Vec::new(),
        });
        self
    }

    pub fn rule_with<F>(mut self, rule: R, start: usize, end: usize, build_children: F) -> Self
    where
        F: FnOnce(PairsBuilder<'i, R>) -> PairsBuilder<'i, R>,
    {
        let children = build_children(PairsBuilder::new(self.input)).nodes;
        self.nodes.push(BuilderNode {
            rule,
            start,
            end,
            tag: None,
            children,
        });
        self
    }

    pub fn tag(mut self, tag: &'i str) -> Self {
        self.nodes
            .last_mut()
            .expect("PairsBuilder::tag called before any rule was added")
            .tag = Some(tag);
        self
    }

    pub fn build(self) -> Pairs<'i, R> {
        let mut queue = Vec::new();

        for node in &self.nodes {
            push_node(&mut queue, self.input, node);
        }

        let end = queue.len();

        pairs::new(
            Rc::new(queue),
            self.input,
            Some(Rc::new(LineIndex::new(self.input))),
            0,
            end,
        )
    }
}

fn push_node<'i, R: RuleType>(
    queue: &mut Vec<QueueableToken<'i, R>>,
    input: &'i str,
    node: &BuilderNode<'i, R>,
) {
    assert!(
        input.get(node.start..node.end).is_some(),
        "PairsBuilder: invalid span {}..{} for input of length {}; \
         start..end must be an ascending range on UTF-8 character boundaries",
        node.start,
        node.end,
        input.len()
    );

    let start_index = queue.len();
    queue.push(QueueableToken::Start {
        end_token_index: 0,
        input_pos: node.start,
    });

    for child in &node.children {
        push_node(queue, input, child);
    }

    let end_index = queue.len();
    match queue[start_index] {
        QueueableToken::Start {
            ref mut end_token_index,
            ..
        } => *end_token_index = end_index,
        // We only ever push a `Start` at `start_index` just above.
        _ => unreachable!(),
    }

    queue.push(QueueableToken::End {
        start_token_index: start_index,
        rule: node.rule,
        tag: node.tag,
        input_pos: node.end,
    });
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/peg/iterators/pairs_builder/tests.rs"]
mod tests;
