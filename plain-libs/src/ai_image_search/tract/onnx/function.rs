use super::super::{control_flow, model::Graph, ops::Op};
use super::proto::{
    AttributeProto, FunctionProto, GraphProto, NodeProto, OperatorSetIdProto, ValueInfoProto,
};
use std::collections::HashMap;

pub(in crate::ai_image_search::tract) struct ParsingContext<'a> {
    functions: &'a [FunctionProto],
    opsets: &'a [OperatorSetIdProto],
    active: Vec<usize>,
    pub symbols: &'a HashMap<String, i64>,
}

impl<'a> ParsingContext<'a> {
    pub(in crate::ai_image_search::tract) fn new(
        functions: &'a [FunctionProto],
        opsets: &'a [OperatorSetIdProto],
        symbols: &'a HashMap<String, i64>,
    ) -> Self {
        Self {
            functions,
            opsets,
            active: Vec::new(),
            symbols,
        }
    }

    pub(in crate::ai_image_search::tract) fn op(&self, node: &mut NodeProto) -> Result<Op, String> {
        fn domain(s: &str) -> &str {
            if s == "ai.onnx" { "" } else { s }
        }
        let function = self.functions.iter().enumerate().find(|(_, f)| {
            f.name.as_deref() == Some(&node.op_type)
                && domain(f.domain.as_deref().unwrap_or_default()) == domain(&node.domain)
                && f.overload == node.overload
        });
        if let Some((index, function)) = function {
            if self.active.contains(&index) {
                return Err(format!("Recursive ONNX function {}", node.op_type));
            }
            if node.input.len() > function.input.len() || node.output.len() > function.output.len()
            {
                return Err(format!(
                    "Invalid {} function inputs or outputs",
                    node.op_type
                ));
            }
            let mut attributes: HashMap<_, _> = function
                .attribute_proto
                .iter()
                .map(|a| (a.name.as_str(), a))
                .collect();
            for attr in &node.attribute {
                attributes.insert(attr.name.as_str(), attr);
            }
            let mut nodes = function.node.clone();
            bind_attributes(&mut nodes, &attributes)?;
            let named = |name: &String| ValueInfoProto {
                name: name.clone(),
                ..Default::default()
            };
            let graph = GraphProto {
                node: nodes,
                input: function.input.iter().map(named).collect(),
                output: function
                    .output
                    .iter()
                    .take(node.output.len())
                    .map(named)
                    .collect(),
                value_info: function.value_info.clone(),
                ..Default::default()
            };
            let mut active = self.active.clone();
            active.push(index);
            let context = ParsingContext {
                functions: self.functions,
                opsets: &function.opset_import,
                active,
                symbols: self.symbols,
            };
            node.input.resize(function.input.len(), String::new());
            let (graph, _) = Graph::body(graph, &context, false)?;
            return Ok(Op::Function(Box::new(graph)));
        }
        let version = self
            .opsets
            .iter()
            .find(|op| domain(&op.domain) == domain(&node.domain))
            .map_or(1, |op| op.version);
        control_flow::parse(node, version, self)
    }
}

fn bind_attributes(
    nodes: &mut [NodeProto],
    bindings: &HashMap<&str, &AttributeProto>,
) -> Result<(), String> {
    for node in nodes {
        let mut attributes = Vec::with_capacity(node.attribute.len());
        for mut attr in std::mem::take(&mut node.attribute) {
            if !attr.ref_attr_name.is_empty() {
                let Some(value) = bindings.get(attr.ref_attr_name.as_str()) else {
                    continue;
                };
                if attr.r#type != value.r#type {
                    return Err(format!(
                        "Unexpected function attribute type for {}",
                        attr.name
                    ));
                }
                let name = attr.name;
                attr = (*value).clone();
                attr.name = name;
            } else if let Some(graph) = &mut attr.g {
                bind_attributes(&mut graph.node, bindings)?;
            }
            attributes.push(attr);
        }
        node.attribute = attributes;
    }
    Ok(())
}
