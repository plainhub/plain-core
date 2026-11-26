use super::infer::InferenceFact;
use super::onnx::{
    self,
    proto::{GraphProto, ValueInfoProto},
};
use super::ops::Op;
use super::plan::Plan;
use super::tensor::{Tensor, array};
use crate::ai_image_search::{Input, Values};
use std::{collections::HashSet, path::Path};

pub(super) struct Model {
    graph: Graph,
    input_order: Vec<usize>,
}

pub(super) struct Graph {
    steps: Vec<Step>,
    outputs: Vec<usize>,
    constants: Vec<(usize, Tensor)>,
    facts: Vec<InferenceFact>,
    inputs: Vec<usize>,
}

struct Step {
    op: Op,
    inputs: Vec<Option<usize>>,
    outputs: Vec<Option<usize>>,
    release: Vec<usize>,
}

impl Model {
    pub(super) fn open(path: &Path, inputs: &[Input<'_>], output: &str) -> Result<Self, String> {
        let model = onnx::read(path)?;
        let graph = model.graph.ok_or("ONNX model has no graph")?;
        let bindings = graph.bind(inputs, output)?;
        let context =
            onnx::ParsingContext::new(&model.functions, &model.opset_import, &bindings.symbols);
        let input_names = bindings
            .input_order
            .iter()
            .map(|&i| inputs[i].name.to_owned())
            .collect();
        let (graph, _) = Graph::compile(graph, &[bindings.output], &context, input_names, false)?;
        Ok(Self {
            graph,
            input_order: bindings.input_order,
        })
    }

    pub(super) fn run(&self, inputs: &mut [Input<'_>]) -> Result<Tensor, String> {
        let feeds = self
            .input_order
            .iter()
            .map(|&index| {
                let input = &mut inputs[index];
                let shape = input
                    .shape
                    .iter()
                    .map(|&n| usize::try_from(n).map_err(|e| e.to_string()))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Some(match &mut input.values {
                    Values::Float(v) => Tensor::F32(array(&shape, std::mem::take(v))?),
                    Values::Integer(v) => Tensor::I64(array(&shape, std::mem::take(v))?),
                }))
            })
            .collect::<Result<Vec<_>, String>>()?;
        self.graph
            .eval(feeds)?
            .pop()
            .ok_or_else(|| "Missing encoder output".into())
    }
}

impl Graph {
    pub(super) fn body(
        graph: GraphProto,
        context: &onnx::ParsingContext,
        nested: bool,
    ) -> Result<(Self, Vec<String>), String> {
        let inputs = graph.input.iter().map(|i| i.name.clone()).collect();
        let outputs = (0..graph.output.len()).collect::<Vec<_>>();
        Self::compile(graph, &outputs, context, inputs, nested)
    }

    fn compile(
        mut graph: GraphProto,
        outputs: &[usize],
        context: &onnx::ParsingContext,
        mut input_names: Vec<String>,
        nested: bool,
    ) -> Result<(Self, Vec<String>), String> {
        let mut ops = Vec::with_capacity(graph.node.len());
        for node in &mut graph.node {
            ops.push(context.op(node)?);
        }
        let mut captures = Vec::new();
        if nested {
            let mut known: HashSet<&str> = graph
                .input
                .iter()
                .map(|v| v.name.as_str())
                .chain(graph.initializer.iter().map(|v| v.name.as_str()))
                .chain(
                    graph
                        .sparse_initializer
                        .iter()
                        .filter_map(|v| v.values.as_ref())
                        .map(|v| v.name.as_str()),
                )
                .chain(
                    graph
                        .node
                        .iter()
                        .flat_map(|n| n.output.iter().map(String::as_str)),
                )
                .collect();
            for name in graph
                .node
                .iter()
                .flat_map(|n| &n.input)
                .chain(graph.output.iter().map(|v| &v.name))
            {
                if !name.is_empty() && known.insert(name) {
                    captures.push(name.clone());
                }
            }
            for name in &captures {
                graph.input.push(ValueInfoProto {
                    name: name.clone(),
                    ..Default::default()
                });
            }
            input_names.extend_from_slice(&captures);
        }
        super::layer_norm::detect(&mut graph, &mut ops, &input_names);
        let plan = Plan::for_outputs(&graph, outputs)?;
        let mut facts: Vec<_> = (0..plan.names.len())
            .map(|_| InferenceFact::default())
            .collect();
        for declared in graph
            .input
            .iter()
            .chain(&graph.value_info)
            .chain(&graph.output)
        {
            if let Some(outlet) = plan.names.get(&declared.name) {
                let mut fact = InferenceFact::declared(declared);
                fact.set_symbols(context.symbols);
                facts[outlet.slot].unify(fact)?;
            }
        }
        let input_slots = input_names
            .iter()
            .map(|name| plan.names[name].slot)
            .collect();
        let mut live = vec![false; plan.names.len()];
        for &output in &plan.outputs {
            live[output] = true;
        }
        let mut ops: Vec<_> = ops.into_iter().map(Some).collect();
        let mut steps = Vec::with_capacity(plan.order.len());
        for (&node, release) in plan.order.iter().zip(plan.release) {
            let wiring = &plan.nodes[node];
            let inputs = wiring
                .inputs
                .iter()
                .map(|input| {
                    input.map(|v| {
                        live[v.slot] = true;
                        v.slot
                    })
                })
                .collect();
            steps.push(Step {
                op: ops[node].take().unwrap(),
                inputs,
                outputs: wiring.outputs.clone(),
                release,
            });
        }
        let retained_slot = |name: &str| {
            let slot = plan.names[name].slot;
            (live[slot] && !input_names.iter().any(|input| input == name)).then_some(slot)
        };
        let mut constants = Vec::new();
        for tensor in graph.initializer {
            if let Some(slot) = retained_slot(&tensor.name) {
                constants.push((slot, Tensor::from_proto(tensor)?));
            }
        }
        for sparse in graph.sparse_initializer {
            let name = &sparse
                .values
                .as_ref()
                .ok_or("Missing sparse tensor values")?
                .name;
            if let Some(slot) = retained_slot(name) {
                constants.push((slot, Tensor::from_sparse(sparse)?));
            }
        }
        for (slot, tensor) in &constants {
            facts[*slot].unify(InferenceFact::from_tensor(tensor))?;
        }
        for step in &steps {
            for (index, slot) in step.outputs.iter().enumerate() {
                if let Some(slot) = slot
                    && let Some(inferred) = step.op.output_fact(&step.inputs, &facts, index)
                {
                    facts[*slot].unify(inferred)?;
                }
            }
        }
        Ok((
            Self {
                steps,
                outputs: plan.outputs,
                constants,
                facts,
                inputs: input_slots,
            },
            captures,
        ))
    }

    pub(super) fn output_fact(&self, index: usize) -> Option<&InferenceFact> {
        self.outputs.get(index).map(|&slot| &self.facts[slot])
    }

    pub(super) fn eval(&self, inputs: Vec<Option<Tensor>>) -> Result<Vec<Tensor>, String> {
        if inputs.len() != self.inputs.len() {
            return Err("Unexpected ONNX graph input count".into());
        }
        let mut values = vec![None; self.facts.len()];
        for (slot, tensor) in &self.constants {
            values[*slot] = Some(tensor.clone());
        }
        for (&slot, tensor) in self.inputs.iter().zip(inputs) {
            if let Some(tensor) = tensor {
                self.facts[slot].check(tensor.dtype() as i32, tensor.shape())?;
                values[slot] = Some(tensor);
            }
        }
        for (index, step) in self.steps.iter().enumerate() {
            let inputs = step
                .inputs
                .iter()
                .map(|input| input.and_then(|slot| values[slot].as_ref()))
                .collect::<Vec<_>>();
            let outputs = step
                .op
                .eval(&inputs)
                .map_err(|e| format!("ONNX node {index}: {e}"))?;
            if outputs.len() != step.outputs.len() {
                return Err(format!("Invalid output count at ONNX node {index}"));
            }
            for (slot, tensor) in step.outputs.iter().zip(outputs) {
                if let Some(slot) = slot {
                    self.facts[*slot].check(tensor.dtype() as i32, tensor.shape())?;
                    values[*slot] = Some(tensor);
                }
            }
            for &slot in &step.release {
                values[slot] = None;
            }
        }
        self.outputs
            .iter()
            .map(|&slot| {
                values[slot]
                    .clone()
                    .ok_or_else(|| "Missing ONNX graph output".into())
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/ai_image_search/tract/model.rs"]
mod tests;
