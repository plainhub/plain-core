use crate::matchit::{InsertError, MatchError, Params};
use std::cell::UnsafeCell;
use std::cmp::min;
use std::mem;
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone)]
pub(crate) enum NodeType {
    Root,
    Param,
    CatchAll,
    Static,
}
pub struct Node<T> {
    priority: u32,
    wild_child: bool,
    indices: Vec<u8>,
    value: Option<UnsafeCell<T>>,
    pub(crate) param_remapping: ParamRemapping,
    pub(crate) node_type: NodeType,
    pub(crate) prefix: Vec<u8>,
    pub(crate) children: Vec<Self>,
}
unsafe impl<T: Send> Send for Node<T> {}
unsafe impl<T: Sync> Sync for Node<T> {}
impl<T> Node<T> {
    pub fn insert(&mut self, route: impl Into<String>, val: T) -> Result<(), InsertError> {
        let route = route.into().into_bytes();
        let (route, param_remapping) = normalize_params(route)?;
        let mut prefix = route.as_ref();
        self.priority += 1;
        if self.prefix.is_empty() && self.children.is_empty() {
            let last = self.insert_child(prefix, &route, val)?;
            last.param_remapping = param_remapping;
            self.node_type = NodeType::Root;
            return Ok(());
        }
        let mut current = self;
        'walk: loop {
            let len = min(prefix.len(), current.prefix.len());
            let common_prefix = (0..len)
                .find(|&i| prefix[i] != current.prefix[i])
                .unwrap_or(len);
            if common_prefix < current.prefix.len() {
                let child = Node {
                    prefix: current.prefix[common_prefix..].to_owned(),
                    children: mem::take(&mut current.children),
                    wild_child: current.wild_child,
                    indices: current.indices.clone(),
                    value: current.value.take(),
                    param_remapping: mem::take(&mut current.param_remapping),
                    priority: current.priority - 1,
                    ..Node::default()
                };
                current.children = vec![child];
                current.indices = vec![current.prefix[common_prefix]];
                current.prefix = prefix[..common_prefix].to_owned();
                current.wild_child = false;
            }
            if prefix.len() > common_prefix {
                prefix = &prefix[common_prefix..];
                let next = prefix[0];
                if current.node_type == NodeType::Param
                    && next == b'/'
                    && current.children.len() == 1
                {
                    current = &mut current.children[0];
                    current.priority += 1;
                    continue 'walk;
                }
                for mut i in 0..current.indices.len() {
                    if next == current.indices[i] {
                        i = current.update_child_priority(i);
                        current = &mut current.children[i];
                        continue 'walk;
                    }
                }
                if !matches!(next, b':' | b'*') && current.node_type != NodeType::CatchAll {
                    current.indices.push(next);
                    let mut child = current.add_child(Node::default());
                    child = current.update_child_priority(child);
                    let last = current.children[child].insert_child(prefix, &route, val)?;
                    last.param_remapping = param_remapping;
                    return Ok(());
                }
                if current.wild_child {
                    current = current.children.last_mut().unwrap();
                    current.priority += 1;
                    if prefix.len() < current.prefix.len()
                        || current.prefix != prefix[..current.prefix.len()]
                        || current.node_type == NodeType::CatchAll
                        || (current.prefix.len() < prefix.len()
                            && prefix[current.prefix.len()] != b'/')
                    {
                        return Err(InsertError::conflict(&route, prefix, current));
                    }
                    continue 'walk;
                }
                let last = current.insert_child(prefix, &route, val)?;
                last.param_remapping = param_remapping;
                return Ok(());
            }
            if current.value.is_some() {
                return Err(InsertError::conflict(&route, prefix, current));
            }
            current.value = Some(UnsafeCell::new(val));
            current.param_remapping = param_remapping;
            return Ok(());
        }
    }
    fn add_child(&mut self, child: Node<T>) -> usize {
        let len = self.children.len();
        if self.wild_child && len > 0 {
            self.children.insert(len - 1, child);
            len - 1
        } else {
            self.children.push(child);
            len
        }
    }
    fn update_child_priority(&mut self, i: usize) -> usize {
        self.children[i].priority += 1;
        let priority = self.children[i].priority;
        let mut updated = i;
        while updated > 0 && self.children[updated - 1].priority < priority {
            self.children.swap(updated - 1, updated);
            updated -= 1;
        }
        if updated != i {
            self.indices = [
                &self.indices[..updated],
                &self.indices[i..=i],
                &self.indices[updated..i],
                &self.indices[i + 1..],
            ]
            .concat();
        }
        updated
    }
    fn insert_child(
        &mut self,
        mut prefix: &[u8],
        route: &[u8],
        val: T,
    ) -> Result<&mut Node<T>, InsertError> {
        let mut current = self;
        loop {
            let (wildcard, wildcard_index) = match find_wildcard(prefix)? {
                Some((w, i)) => (w, i),
                None => {
                    current.value = Some(UnsafeCell::new(val));
                    current.prefix = prefix.to_owned();
                    return Ok(current);
                }
            };
            if wildcard[0] == b':' {
                if wildcard_index > 0 {
                    current.prefix = prefix[..wildcard_index].to_owned();
                    prefix = &prefix[wildcard_index..];
                }
                let child = Self {
                    node_type: NodeType::Param,
                    prefix: wildcard.to_owned(),
                    ..Self::default()
                };
                let child = current.add_child(child);
                current.wild_child = true;
                current = &mut current.children[child];
                current.priority += 1;
                if wildcard.len() < prefix.len() {
                    prefix = &prefix[wildcard.len()..];
                    let child = Self {
                        priority: 1,
                        ..Self::default()
                    };
                    let child = current.add_child(child);
                    current = &mut current.children[child];
                    continue;
                }
                current.value = Some(UnsafeCell::new(val));
                return Ok(current);
            } else if wildcard[0] == b'*' {
                if wildcard_index + wildcard.len() != prefix.len() {
                    return Err(InsertError::InvalidCatchAll);
                }
                if let Some(i) = wildcard_index.checked_sub(1) {
                    if prefix[i] != b'/' {
                        return Err(InsertError::InvalidCatchAll);
                    }
                }
                if prefix == route && route[0] != b'/' {
                    return Err(InsertError::InvalidCatchAll);
                }
                if wildcard_index > 0 {
                    current.prefix = prefix[..wildcard_index].to_owned();
                    prefix = &prefix[wildcard_index..];
                }
                let child = Self {
                    prefix: prefix.to_owned(),
                    node_type: NodeType::CatchAll,
                    value: Some(UnsafeCell::new(val)),
                    priority: 1,
                    ..Self::default()
                };
                let i = current.add_child(child);
                current.wild_child = true;
                return Ok(&mut current.children[i]);
            }
        }
    }
}
struct Skipped<'n, 'p, T> {
    path: &'p [u8],
    node: &'n Node<T>,
    params: usize,
}
#[rustfmt::skip] macro_rules! backtracker { ($skipped_nodes : ident , $path : ident , $current : ident , $params : ident , $backtracking : ident , $walk : lifetime) => { macro_rules! try_backtrack { () => { while let Some (skipped) = $skipped_nodes . pop () { if skipped . path . ends_with ($path) { $path = skipped . path ; $current = & skipped . node ; $params . truncate (skipped . params) ; $backtracking = true ; continue $walk ; } } } ; } } ; }
impl<T> Node<T> {
    pub fn at<'n, 'p>(
        &'n self,
        full_path: &'p [u8],
    ) -> Result<(&'n UnsafeCell<T>, Params<'n, 'p>), MatchError> {
        let mut current = self;
        let mut path = full_path;
        let mut backtracking = false;
        let mut params = Params::new();
        let mut skipped_nodes = Vec::new();
        'walk: loop {
            backtracker ! (skipped_nodes , path , current , params , backtracking , 'walk);
            if path.len() > current.prefix.len() {
                let (prefix, rest) = path.split_at(current.prefix.len());
                if prefix == current.prefix {
                    let first = rest[0];
                    let consumed = path;
                    path = rest;
                    if !backtracking {
                        if let Some(i) = current.indices.iter().position(|&c| c == first) {
                            if current.wild_child {
                                skipped_nodes.push(Skipped {
                                    path: consumed,
                                    node: current,
                                    params: params.len(),
                                });
                            }
                            if path == b"/"
                                && current.children[i].prefix != b"/"
                                && current.value.is_some()
                            {
                                return Err(MatchError::ExtraTrailingSlash);
                            }
                            current = &current.children[i];
                            continue 'walk;
                        }
                    }
                    if !current.wild_child {
                        if path == b"/" && current.value.is_some() {
                            return Err(MatchError::ExtraTrailingSlash);
                        }
                        if path != b"/" {
                            try_backtrack!();
                        }
                        return Err(MatchError::NotFound);
                    }
                    current = current.children.last().unwrap();
                    match current.node_type {
                        NodeType::Param => match path.iter().position(|&c| c == b'/') {
                            Some(i) => {
                                let (param, rest) = path.split_at(i);
                                if let [child] = current.children.as_slice() {
                                    if rest == b"/"
                                        && child.prefix != b"/"
                                        && current.value.is_some()
                                    {
                                        return Err(MatchError::ExtraTrailingSlash);
                                    }
                                    params.push(&current.prefix[1..], param);
                                    path = rest;
                                    current = child;
                                    backtracking = false;
                                    continue 'walk;
                                }
                                if path.len() == i + 1 {
                                    return Err(MatchError::ExtraTrailingSlash);
                                }
                                if path != b"/" {
                                    try_backtrack!();
                                }
                                return Err(MatchError::NotFound);
                            }
                            None => {
                                params.push(&current.prefix[1..], path);
                                if let Some(ref value) = current.value {
                                    params.for_each_key_mut(|(i, key)| {
                                        *key = &current.param_remapping[i][1..]
                                    });
                                    return Ok((value, params));
                                }
                                if let [child] = current.children.as_slice() {
                                    current = child;
                                    if (current.prefix == b"/" && current.value.is_some())
                                        || (current.prefix.is_empty() && current.indices == b"/")
                                    {
                                        return Err(MatchError::MissingTrailingSlash);
                                    }
                                    if path != b"/" {
                                        try_backtrack!();
                                    }
                                }
                                return Err(MatchError::NotFound);
                            }
                        },
                        NodeType::CatchAll => {
                            return match current.value {
                                Some(ref value) => {
                                    params.for_each_key_mut(|(i, key)| {
                                        *key = &current.param_remapping[i][1..]
                                    });
                                    params.push(&current.prefix[1..], path);
                                    Ok((value, params))
                                }
                                None => Err(MatchError::NotFound),
                            };
                        }
                        _ => unreachable!(),
                    }
                }
            }
            if path == current.prefix {
                if let Some(ref value) = current.value {
                    params.for_each_key_mut(|(i, key)| *key = &current.param_remapping[i][1..]);
                    return Ok((value, params));
                }
                if path != b"/" {
                    try_backtrack!();
                }
                if path == b"/" && current.wild_child && current.node_type != NodeType::Root {
                    return Err(MatchError::unsure(full_path));
                }
                if !backtracking {
                    if let Some(i) = current.indices.iter().position(|&c| c == b'/') {
                        current = &current.children[i];
                        if current.prefix.len() == 1 && current.value.is_some() {
                            return Err(MatchError::MissingTrailingSlash);
                        }
                    }
                }
                return Err(MatchError::NotFound);
            }
            if current.prefix.split_last() == Some((&b'/', path)) && current.value.is_some() {
                return Err(MatchError::MissingTrailingSlash);
            }
            if path != b"/" {
                try_backtrack!();
            }
            return Err(MatchError::NotFound);
        }
    }
}
type ParamRemapping = Vec<Vec<u8>>;
fn normalize_params(mut path: Vec<u8>) -> Result<(Vec<u8>, ParamRemapping), InsertError> {
    let mut start = 0;
    let mut original = ParamRemapping::new();
    let mut next = b'a';
    loop {
        let (wildcard, mut wildcard_index) = match find_wildcard(&path[start..])? {
            Some((w, i)) => (w, i),
            None => return Ok((path, original)),
        };
        if wildcard.len() < 2 {
            return Err(InsertError::UnnamedParam);
        }
        if wildcard[0] == b'*' {
            start += wildcard_index + wildcard.len();
            continue;
        }
        wildcard_index += start;
        let removed = path.splice(
            (wildcard_index)..(wildcard_index + wildcard.len()),
            vec![b':', next],
        );
        original.push(removed.collect());
        next += 1;
        if next > b'z' {
            panic!("too many route parameters");
        }
        start = wildcard_index + 2;
    }
}
pub(crate) fn denormalize_params(route: &mut Vec<u8>, params: &ParamRemapping) {
    let mut start = 0;
    let mut i = 0;
    loop {
        let (wildcard, mut wildcard_index) = match find_wildcard(&route[start..]).unwrap() {
            Some((w, i)) => (w, i),
            None => return,
        };
        wildcard_index += start;
        let next = match params.get(i) {
            Some(param) => param.clone(),
            None => return,
        };
        route.splice(
            (wildcard_index)..(wildcard_index + wildcard.len()),
            next.clone(),
        );
        i += 1;
        start = wildcard_index + 2;
    }
}
fn find_wildcard(path: &[u8]) -> Result<Option<(&[u8], usize)>, InsertError> {
    for (start, &c) in path.iter().enumerate() {
        if c != b':' && c != b'*' {
            continue;
        }
        for (end, &c) in path[start + 1..].iter().enumerate() {
            match c {
                b'/' => return Ok(Some((&path[start..start + 1 + end], start))),
                b':' | b'*' => return Err(InsertError::TooManyParams),
                _ => {}
            }
        }
        return Ok(Some((&path[start..], start)));
    }
    Ok(None)
}
impl<T> Clone for Node<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        let value = self.value.as_ref().map(|value| {
            let value = unsafe { &*value.get() };
            UnsafeCell::new(value.clone())
        });
        Self {
            value,
            prefix: self.prefix.clone(),
            wild_child: self.wild_child,
            node_type: self.node_type.clone(),
            indices: self.indices.clone(),
            children: self.children.clone(),
            param_remapping: self.param_remapping.clone(),
            priority: self.priority,
        }
    }
}
impl<T> Default for Node<T> {
    fn default() -> Self {
        Self {
            param_remapping: ParamRemapping::new(),
            prefix: Vec::new(),
            wild_child: false,
            node_type: NodeType::Static,
            indices: Vec::new(),
            children: Vec::new(),
            value: None,
            priority: 0,
        }
    }
}
