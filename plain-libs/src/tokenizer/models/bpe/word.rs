use super::Pair;
use std::collections::HashMap;
use std::{cmp::Ordering, collections::BinaryHeap};

#[derive(Debug, Eq)]
struct Merge {
    pos: usize,
    rank: u32,
    new_id: u32,
}

impl PartialEq for Merge {
    fn eq(&self, other: &Self) -> bool {
        self.rank == other.rank && self.pos == other.pos
    }
}

impl PartialOrd for Merge {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // By manually implementing this, we make the containing BinaryHeap a
        // min-heap ordered first on the rank, and the pos otherwise
        Some(self.cmp(other))
    }
}

impl Ord for Merge {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.rank != other.rank {
            other.rank.cmp(&self.rank)
        } else {
            other.pos.cmp(&self.pos)
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Symbol {
    c: u32,
    prev: isize,
    next: isize,
    len: usize,
}
impl Symbol {
    pub fn merge_with(&mut self, other: &Self, new_c: u32) {
        self.c = new_c;
        self.len += other.len;
        self.next = other.next;
    }
}

#[derive(Clone, Default)]
pub(super) struct Word {
    symbols: Vec<Symbol>,
}
impl std::fmt::Debug for Word {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("Word")
            .field(
                "chars",
                &self
                    .symbols
                    .iter()
                    .map(|s| s.c.to_string())
                    .collect::<Vec<_>>()
                    .join(" "),
            )
            .field("symbols", &self.symbols)
            .finish()
    }
}

impl Word {
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            symbols: Vec::with_capacity(capacity),
        }
    }

    pub(super) fn add(&mut self, c: u32, byte_len: usize) {
        let (prev, next) = {
            let len = self.symbols.len() as isize;
            if let Some(last) = self.symbols.last_mut() {
                // Update `next` on the previous one
                last.next = len;
                (len - 1, -1)
            } else {
                (-1, -1)
            }
        };
        self.symbols.push(Symbol {
            c,
            prev,
            next,
            len: byte_len,
        });
    }

    pub(super) fn merge_all(&mut self, merges: &HashMap<Pair, (u32, u32)>, dropout: Option<f32>) {
        let mut queue = BinaryHeap::with_capacity(self.symbols.len());
        let mut skip = Vec::with_capacity(queue.len());

        queue.extend(
            self.symbols
                .windows(2)
                .enumerate()
                .filter_map(|(index, window)| {
                    let pair = (window[0].c, window[1].c);
                    merges.get(&pair).map(|m| Merge {
                        pos: index,
                        rank: m.0,
                        new_id: m.1,
                    })
                }),
        );

        while let Some(top) = queue.pop() {
            if dropout.is_some_and(|probability| rand::random::<f32>() < probability) {
                skip.push(top);
            } else {
                // Re-insert the skipped elements
                queue.extend(skip.drain(..));

                if self.symbols[top.pos].len == 0 {
                    continue;
                }
                // Do nothing if we are the last symbol
                if self.symbols[top.pos].next == -1 {
                    continue;
                }

                let next_pos = self.symbols[top.pos].next as usize;
                let right = self.symbols[next_pos];

                // Make sure we are not processing an expired queue entry
                let target_new_pair = (self.symbols[top.pos].c, right.c);
                if merges
                    .get(&target_new_pair)
                    .is_none_or(|(_, new_id)| *new_id != top.new_id)
                {
                    continue;
                }

                // Otherwise, let's merge
                self.symbols[top.pos].merge_with(&right, top.new_id);
                // Tag the right part as removed
                self.symbols[next_pos].len = 0;

                // Update `prev` on the new `next` to the current pos
                if right.next > -1 && (right.next as usize) < self.symbols.len() {
                    self.symbols[right.next as usize].prev = top.pos as isize;
                }

                // Insert the new pair formed with the previous symbol
                let current = &self.symbols[top.pos];
                if current.prev >= 0 {
                    let prev = current.prev as usize;
                    let prev_symbol = self.symbols[prev];
                    let new_pair = (prev_symbol.c, current.c);
                    if let Some((rank, new_id)) = merges.get(&new_pair) {
                        queue.push(Merge {
                            pos: current.prev as usize,
                            rank: *rank,
                            new_id: *new_id,
                        });
                    }
                }

                // Insert the new pair formed with the next symbol
                let next = current.next as usize;
                if next < self.symbols.len() {
                    let next_symbol = self.symbols[next];
                    let new_pair = (current.c, next_symbol.c);
                    if let Some((rank, new_id)) = merges.get(&new_pair) {
                        queue.push(Merge {
                            pos: top.pos,
                            rank: *rank,
                            new_id: *new_id,
                        });
                    }
                }
            }
        }

        // Filter out the removed symbols
        self.symbols.retain(|s| s.len != 0);
    }

    pub(super) fn get_chars_iter(&self) -> impl Iterator<Item = u32> + '_ {
        self.symbols.iter().map(|s| s.c)
    }

    pub(super) fn get_offsets_iter(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        let mut pos = 0;
        self.symbols.iter().map(move |symbol| {
            let new_pos = pos + symbol.len;
            let offset = (pos, new_pos);
            pos = new_pos;
            offset
        })
    }
}
