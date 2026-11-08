use std::cell::RefCell;
use std::rc::Rc;

type NodeRef = Rc<RefCell<Node>>;

#[derive(Debug)]
pub(super) struct Lattice<'a> {
    pub(super) sentence: &'a str,
    len: usize,
    pub(super) begin_nodes: Vec<Vec<NodeRef>>,
    pub(super) end_nodes: Vec<Vec<NodeRef>>,
}

#[derive(Debug, Clone)]
pub(super) struct Node {
    // Vocabulary id
    pub(super) id: usize,
    pos: usize,
    length: usize,
    prev: Option<NodeRef>,
    backtrace_score: f64,
    score: f64,
}

#[cfg(test)]
impl PartialEq for Node {
    fn eq(&self, other: &Node) -> bool {
        self.id == other.id
    }
}

impl Node {
    fn new(id: usize, pos: usize, length: usize, score: f64) -> Self {
        Self {
            id,
            pos,
            length,
            prev: None,
            score,
            backtrace_score: 0.0,
        }
    }
}

impl<'a> Lattice<'a> {
    pub fn from(sentence: &'a str, bos_id: usize, eos_id: usize) -> Self {
        let len = sentence.len();
        let k_reserved_node_size = 16;
        let mut begin_nodes = vec![Vec::with_capacity(k_reserved_node_size); len + 1];
        let mut end_nodes = vec![Vec::with_capacity(k_reserved_node_size); len + 1];

        let bos = Rc::new(RefCell::new(Node::new(bos_id, 0, 0, 0.0)));
        let eos = Rc::new(RefCell::new(Node::new(eos_id, len, 0, 0.0)));

        begin_nodes[len].push(eos);
        end_nodes[0].push(bos);

        Self {
            sentence,
            len,
            begin_nodes,
            end_nodes,
        }
    }

    pub fn insert(&mut self, pos: usize, length: usize, score: f64, id: usize) {
        let node = Rc::new(RefCell::new(Node::new(id, pos, length, score)));

        self.begin_nodes[pos].push(Rc::clone(&node));
        self.end_nodes[pos + length].push(node);
    }

    pub fn viterbi(&mut self) -> Vec<NodeRef> {
        let len = self.len;
        let mut pos = 0;
        while pos <= len {
            if self.begin_nodes[pos].is_empty() {
                return vec![];
            }
            for rnode in &self.begin_nodes[pos] {
                rnode.borrow_mut().prev = None;
                let mut best_score = 0.0;
                let mut best_node: Option<NodeRef> = None;
                for lnode in &self.end_nodes[pos] {
                    let score = lnode.borrow().backtrace_score + rnode.borrow().score;
                    if best_node.is_none() || score > best_score {
                        // TODO can we remove this clone ?
                        best_node = Some(lnode.clone());
                        best_score = score
                    }
                }
                match best_node {
                    Some(bnode) => {
                        rnode.borrow_mut().prev = Some(Rc::clone(&bnode));
                        rnode.borrow_mut().backtrace_score = best_score;
                    }
                    None => return vec![],
                }
            }
            if let Some(c) = self.sentence[pos..].chars().next() {
                pos += c.len_utf8();
            } else {
                break;
            }
        }

        let mut results: Vec<NodeRef> = vec![];
        let root = self.begin_nodes[len][0].borrow();
        let prev = root.prev.as_ref();
        if prev.is_none() {
            return vec![];
        }
        let mut node: NodeRef = prev.unwrap().clone();
        while node.borrow().prev.is_some() {
            results.push(node.clone());
            let n = node.borrow().clone();
            node = n.prev.as_ref().unwrap().clone();
        }
        results.reverse();
        results
    }

    pub fn piece(&self, node: &Node) -> String {
        self.sentence[node.pos..node.pos + node.length].to_owned()
    }

    pub fn tokens(&mut self) -> Vec<String> {
        self.viterbi()
            .iter()
            .map(|node| self.piece(&node.borrow()))
            .collect()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    #[cfg(test)]
    fn bos_node(&self) -> NodeRef {
        Rc::clone(&self.end_nodes[0][0])
    }
    #[cfg(test)]
    fn eos_node(&self) -> NodeRef {
        Rc::clone(&self.begin_nodes[self.len][0])
    }

    #[cfg(test)]
    fn surface(&self, n: usize) -> &str {
        match self.sentence.char_indices().nth(n) {
            Some((pos, _)) => &self.sentence[pos..],
            None => "",
        }
    }
    #[cfg(test)]
    fn sentence(&self) -> &str {
        self.sentence
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/tokenizer/models/unigram/lattice/tests.rs"]
mod tests;
