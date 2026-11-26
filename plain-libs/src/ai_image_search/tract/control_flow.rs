use super::onnx::ParsingContext;
use super::{
    array_ops::{concat, unsqueeze},
    infer::InferenceFact,
    model::Graph,
    onnx::proto::{NodeProto, attribute_proto::AttributeType},
    ops::Op,
    tensor::{Tensor, array},
};

pub(super) enum ControlFlow {
    If {
        then_branch: Branch,
        else_branch: Branch,
    },
    Loop {
        body: Branch,
        carried: usize,
        scans: usize,
    },
}

pub(super) struct Branch {
    graph: Graph,
    captures: Vec<usize>,
}

pub(super) fn parse(
    node: &mut NodeProto,
    opset: i64,
    context: &ParsingContext,
) -> Result<Op, String> {
    if !matches!(node.op_type.as_str(), "If" | "Loop")
        || !(node.domain.is_empty() || node.domain == "ai.onnx")
    {
        return Op::from_node(node, opset);
    }
    let explicit = node.input.len();
    let mut capture_names = Vec::<String>::new();
    let mut branch = |name: &str, inputs: usize, outputs: usize| -> Result<Branch, String> {
        let body = node
            .attribute
            .iter_mut()
            .find(|a| a.name == name && a.r#type == AttributeType::Graph as i32)
            .and_then(|a| a.g.take())
            .ok_or_else(|| format!("Missing ONNX subgraph {name}"))?;
        if body.input.len() != inputs || body.output.len() != outputs {
            return Err(format!(
                "Invalid input/output count for ONNX subgraph {name}"
            ));
        }
        let (graph, names) = Graph::body(body, context, true)?;
        let mut captures = Vec::with_capacity(names.len());
        for name in names {
            let i = if let Some(i) = capture_names.iter().position(|v| *v == name) {
                i
            } else {
                capture_names.push(name);
                capture_names.len() - 1
            };
            captures.push(explicit + i);
        }
        Ok(Branch { graph, captures })
    };
    let op = match node.op_type.as_str() {
        "If" if explicit == 1 => ControlFlow::If {
            then_branch: branch("then_branch", 0, node.output.len())?,
            else_branch: branch("else_branch", 0, node.output.len())?,
        },
        "Loop" if explicit >= 2 => {
            let carried = explicit - 2;
            let scans = node
                .output
                .len()
                .checked_sub(carried)
                .ok_or("Too few Loop outputs")?;
            ControlFlow::Loop {
                body: branch("body", explicit, node.output.len() + 1)?,
                carried,
                scans,
            }
        }
        _ => return Err(format!("Invalid {} inputs", node.op_type)),
    };
    node.input.extend(capture_names);
    Ok(Op::Control(Box::new(op)))
}

fn condition(input: &Tensor) -> Result<bool, String> {
    match input {
        Tensor::Bool(v) if v.len() == 1 => Ok(*v.first().unwrap()),
        _ => Err("Expected an ONNX boolean condition".into()),
    }
}

impl Branch {
    fn eval(
        &self,
        mut arguments: Vec<Tensor>,
        inputs: &[Option<&Tensor>],
    ) -> Result<Vec<Tensor>, String> {
        for &index in &self.captures {
            arguments.push(inputs[index].ok_or("Missing captured ONNX value")?.clone());
        }
        self.graph.eval(arguments.into_iter().map(Some).collect())
    }
}

impl ControlFlow {
    pub(super) fn eval(&self, inputs: &[Option<&Tensor>]) -> Result<Vec<Tensor>, String> {
        match self {
            Self::If {
                then_branch,
                else_branch,
            } => {
                let selected = if condition(inputs[0].ok_or("Missing If condition")?)? {
                    then_branch
                } else {
                    else_branch
                };
                selected.eval(Vec::new(), inputs)
            }
            Self::Loop {
                body,
                carried,
                scans,
            } => {
                let max = match inputs[0] {
                    Some(Tensor::I64(v)) if v.len() == 1 => *v.first().unwrap(),
                    None => i64::MAX,
                    _ => return Err("Expected an integer Loop trip count".into()),
                };
                let mut keep = inputs[1].map(condition).transpose()?.unwrap_or(true);
                let mut state = inputs[2..2 + carried]
                    .iter()
                    .map(|v| v.cloned().ok_or("Missing Loop carried input"))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut scan_outputs = vec![Vec::new(); *scans];
                for iteration in 0..max {
                    if !keep {
                        break;
                    }
                    let mut args = vec![
                        Tensor::I64(array(&[], vec![iteration])?),
                        Tensor::Bool(array(&[], vec![keep])?),
                    ];
                    args.append(&mut state);
                    let mut result = body.eval(args, inputs)?.into_iter();
                    keep = condition(&result.next().ok_or("Missing Loop condition")?)?;
                    state.extend(result.by_ref().take(*carried));
                    for (scan, value) in scan_outputs.iter_mut().zip(result) {
                        scan.push(unsqueeze(&value, &[0])?);
                    }
                }
                for (index, scan) in scan_outputs.into_iter().enumerate() {
                    let value = if scan.is_empty() {
                        body.graph
                            .output_fact(1 + carried + index)
                            .and_then(InferenceFact::empty_scan)
                            .ok_or("Missing Loop scan output type")??
                    } else {
                        concat(&scan.iter().collect::<Vec<_>>(), 0)?
                    };
                    state.push(value);
                }
                Ok(state)
            }
        }
    }
}
