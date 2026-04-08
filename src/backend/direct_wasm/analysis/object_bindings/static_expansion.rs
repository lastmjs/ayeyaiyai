use super::*;

pub(in crate::backend::direct_wasm) fn object_binding_to_expression(
    object_binding: &ObjectValueBinding,
) -> Expression {
    let mut entries = Vec::new();
    for (name, value) in &object_binding.string_properties {
        entries.push(ObjectEntry::Data {
            key: Expression::String(name.clone()),
            value: value.clone(),
        });
    }
    for (property, value) in &object_binding.symbol_properties {
        entries.push(ObjectEntry::Data {
            key: property.clone(),
            value: value.clone(),
        });
    }
    Expression::Object(entries)
}

pub(in crate::backend::direct_wasm) fn object_literal_prototype_expression(
    expression: &Expression,
) -> Option<Expression> {
    let Expression::Object(entries) = expression else {
        return None;
    };
    entries.iter().rev().find_map(|entry| match entry {
        crate::ir::hir::ObjectEntry::Data { key, value }
            if matches!(key, Expression::String(name) if name == "__proto__") =>
        {
            Some(value.clone())
        }
        _ => None,
    })
}

pub(in crate::backend::direct_wasm) fn resolve_property_descriptor_definition(
    expression: &Expression,
) -> Option<PropertyDescriptorDefinition> {
    let Expression::Object(entries) = expression else {
        return None;
    };

    let mut descriptor = PropertyDescriptorDefinition::default();
    for entry in entries {
        match entry {
            crate::ir::hir::ObjectEntry::Data { key, value } => {
                let Expression::String(key_name) = key else {
                    return None;
                };
                match key_name.as_str() {
                    "value" => descriptor.value = Some(value.clone()),
                    "writable" => {
                        let Expression::Bool(value) = value else {
                            return None;
                        };
                        descriptor.writable = Some(*value);
                    }
                    "enumerable" => {
                        let Expression::Bool(value) = value else {
                            return None;
                        };
                        descriptor.enumerable = Some(*value);
                    }
                    "configurable" => {
                        let Expression::Bool(value) = value else {
                            return None;
                        };
                        descriptor.configurable = Some(*value);
                    }
                    "get" => descriptor.getter = Some(value.clone()),
                    "set" => descriptor.setter = Some(value.clone()),
                    _ => return None,
                }
            }
            crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                let Expression::String(key_name) = key else {
                    return None;
                };
                if key_name != "get" {
                    return None;
                }
                descriptor.getter = Some(getter.clone());
            }
            crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                let Expression::String(key_name) = key else {
                    return None;
                };
                if key_name != "set" {
                    return None;
                }
                descriptor.setter = Some(setter.clone());
            }
            crate::ir::hir::ObjectEntry::Spread(_) => return None,
        }
    }

    Some(descriptor)
}

pub(in crate::backend::direct_wasm) fn infer_global_expression_kind(
    expression: &Expression,
) -> StaticValueKind {
    match expression {
        Expression::Number(_) => StaticValueKind::Number,
        Expression::BigInt(_) => StaticValueKind::BigInt,
        Expression::String(_) => StaticValueKind::String,
        Expression::Bool(_) => StaticValueKind::Bool,
        Expression::Null => StaticValueKind::Null,
        Expression::Undefined => StaticValueKind::Undefined,
        Expression::Array(_) | Expression::Object(_) => StaticValueKind::Object,
        Expression::Unary {
            op: UnaryOp::TypeOf,
            ..
        } => StaticValueKind::String,
        Expression::Identifier(name) if name == "undefined" => StaticValueKind::Undefined,
        Expression::Identifier(name)
            if builtin_identifier_kind(name) == Some(StaticValueKind::Function) =>
        {
            StaticValueKind::Function
        }
        Expression::Identifier(_) => StaticValueKind::Unknown,
        _ => StaticValueKind::Unknown,
    }
}

pub(in crate::backend::direct_wasm) fn expand_static_array_binding(
    expression: &Expression,
    global_array_bindings: &HashMap<String, ArrayValueBinding>,
) -> Option<ArrayValueBinding> {
    match expression {
        Expression::Identifier(name) => global_array_bindings.get(name).cloned(),
        Expression::Array(elements) => {
            let mut values = Vec::new();
            for element in elements {
                match element {
                    crate::ir::hir::ArrayElement::Expression(expression) => {
                        values.push(Some(expression.clone()));
                    }
                    crate::ir::hir::ArrayElement::Spread(expression) => {
                        if let Some(binding) =
                            expand_static_array_binding(expression, global_array_bindings)
                        {
                            values.extend(binding.values);
                        } else {
                            values.push(Some(expression.clone()));
                        }
                    }
                }
            }
            Some(ArrayValueBinding { values })
        }
        _ => None,
    }
}

pub(in crate::backend::direct_wasm) fn expand_static_call_arguments(
    arguments: &[CallArgument],
    global_array_bindings: &HashMap<String, ArrayValueBinding>,
) -> Vec<Expression> {
    let mut expanded = Vec::new();
    for argument in arguments {
        match argument {
            CallArgument::Expression(expression) => expanded.push(expression.clone()),
            CallArgument::Spread(expression) => {
                if let Some(binding) =
                    expand_static_array_binding(expression, global_array_bindings)
                {
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

pub(in crate::backend::direct_wasm) fn builtin_identifier_kind(
    name: &str,
) -> Option<StaticValueKind> {
    match name {
        "Number" | "String" | "Boolean" | "Symbol" | "BigInt" | "Function" | "Object" | "Array"
        | "ArrayBuffer" | "Date" | "RegExp" | "Map" | "Set" | "Error" | "EvalError"
        | "RangeError" | "ReferenceError" | "SyntaxError" | "TypeError" | "URIError"
        | "AggregateError" | "Uint8Array" | "Int8Array" | "Uint16Array" | "Int16Array"
        | "Uint32Array" | "Int32Array" | "Float32Array" | "Float64Array" | "Uint8ClampedArray"
        | "Promise" | "WeakRef" | "eval" => Some(StaticValueKind::Function),
        "Math" | "JSON" | "globalThis" => Some(StaticValueKind::Object),
        "Infinity" | "NaN" | "undefined" => Some(StaticValueKind::Undefined),
        _ => None,
    }
}
