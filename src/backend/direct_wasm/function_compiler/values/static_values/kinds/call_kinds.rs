use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn expand_call_arguments(
        &self,
        arguments: &[CallArgument],
    ) -> Vec<Expression> {
        let mut expanded = Vec::new();
        for argument in arguments {
            match argument {
                CallArgument::Expression(expression) => expanded.push(expression.clone()),
                CallArgument::Spread(expression) => {
                    if let Some(binding) = self.resolve_array_binding_from_expression(expression) {
                        expanded.extend(
                            binding
                                .values
                                .into_iter()
                                .map(|value| value.unwrap_or(Expression::Undefined)),
                        );
                    } else {
                        expanded.push(expression.clone());
                    }
                }
            }
        }
        expanded
    }

    pub(in crate::backend::direct_wasm) fn infer_call_result_kind(
        &self,
        name: &str,
    ) -> Option<StaticValueKind> {
        match name {
            "Number" => Some(StaticValueKind::Number),
            "String" => Some(StaticValueKind::String),
            "Boolean" => Some(StaticValueKind::Bool),
            "isNaN" => Some(StaticValueKind::Bool),
            "Object" | "Array" | "ArrayBuffer" | "Date" | "RegExp" | "Map" | "Set" | "Error"
            | "EvalError" | "RangeError" | "ReferenceError" | "SyntaxError" | "TypeError"
            | "URIError" | "AggregateError" | "Promise" | "WeakRef" => {
                Some(StaticValueKind::Object)
            }
            "Uint8Array" | "Int8Array" | "Uint16Array" | "Int16Array" | "Uint32Array"
            | "Int32Array" | "Float32Array" | "Float64Array" | "Uint8ClampedArray" => {
                Some(StaticValueKind::Object)
            }
            "BigInt" => Some(StaticValueKind::BigInt),
            "Symbol" => Some(StaticValueKind::Symbol),
            "Function" => Some(StaticValueKind::Function),
            _ => None,
        }
    }
}
