use super::onnx::proto::GraphProto;
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub(super) struct Outlet {
    pub slot: usize,
    pub node: Option<usize>,
}

pub(super) struct Node {
    pub inputs: Vec<Option<Outlet>>,
    pub outputs: Vec<Option<usize>>,
}

pub(super) struct Plan {
    pub nodes: Vec<Node>,
    pub order: Vec<usize>,
    pub release: Vec<Vec<usize>>,
    pub names: HashMap<String, Outlet>,
    pub outputs: Vec<usize>,
}

impl Plan {
    pub(super) fn for_outputs(graph: &GraphProto, outputs: &[usize]) -> Result<Self, String> {
        let mut names = HashMap::<String, Outlet>::new();
        for name in graph
            .input
            .iter()
            .map(|v| &v.name)
            .chain(graph.initializer.iter().map(|v| &v.name))
            .chain(
                graph
                    .sparse_initializer
                    .iter()
                    .filter_map(|v| v.values.as_ref())
                    .map(|v| &v.name),
            )
        {
            let slot = names.len();
            names
                .entry(name.clone())
                .or_insert(Outlet { slot, node: None });
        }
        let mut nodes = Vec::with_capacity(graph.node.len());
        for (index, node) in graph.node.iter().enumerate() {
            let mut outputs = Vec::with_capacity(node.output.len());
            for name in &node.output {
                if name.is_empty() {
                    outputs.push(None);
                } else {
                    let slot = names.len();
                    if names
                        .insert(
                            name.clone(),
                            Outlet {
                                slot,
                                node: Some(index),
                            },
                        )
                        .is_some()
                    {
                        return Err(format!("Duplicate ONNX value {name}"));
                    }
                    outputs.push(Some(slot));
                }
            }
            nodes.push(Node {
                inputs: Vec::new(),
                outputs,
            });
        }
        for (node, source) in nodes.iter_mut().zip(&graph.node) {
            node.inputs = source
                .input
                .iter()
                .map(|name| {
                    if name.is_empty() {
                        Ok(None)
                    } else {
                        names
                            .get(name)
                            .copied()
                            .map(Some)
                            .ok_or_else(|| format!("Unknown ONNX value {name}"))
                    }
                })
                .collect::<Result<_, _>>()?;
        }

        let targets = outputs
            .iter()
            .map(|&index| {
                let name = &graph
                    .output
                    .get(index)
                    .ok_or("Unknown encoder output")?
                    .name;
                names
                    .get(name)
                    .copied()
                    .ok_or_else(|| format!("Missing ONNX output {name}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut order = Vec::with_capacity(nodes.len());
        let mut visiting = vec![false; nodes.len()];
        let mut done = vec![false; nodes.len()];
        for root in targets.iter().filter_map(|target| target.node) {
            if done[root] {
                continue;
            }
            let mut stack = vec![(root, 0usize)];
            visiting[root] = true;
            while let Some((node, index)) = stack.last_mut() {
                if *index == nodes[*node].inputs.len() {
                    order.push(*node);
                    done[*node] = true;
                    visiting[*node] = false;
                    stack.pop();
                } else {
                    let input = nodes[*node].inputs[*index];
                    *index += 1;
                    if let Some(dependency) = input.and_then(|v| v.node) {
                        if visiting[dependency] {
                            return Err("Cycle in ONNX graph".into());
                        }
                        if !done[dependency] {
                            visiting[dependency] = true;
                            stack.push((dependency, 0));
                        }
                    }
                }
            }
        }
        let mut last_use = vec![None; names.len()];
        for (step, &node) in order.iter().enumerate() {
            for input in nodes[node].inputs.iter().flatten() {
                last_use[input.slot] = Some(step);
            }
            for &slot in nodes[node].outputs.iter().flatten() {
                last_use[slot].get_or_insert(step);
            }
        }
        for target in &targets {
            last_use[target.slot] = None;
        }
        let mut release = vec![Vec::new(); order.len()];
        for (slot, step) in last_use.into_iter().enumerate() {
            if let Some(step) = step {
                release[step].push(slot);
            }
        }
        Ok(Self {
            outputs: targets.iter().map(|target| target.slot).collect(),
            names,
            nodes,
            order,
            release,
        })
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/ai_image_search/tract/plan.rs"]
mod tests;
