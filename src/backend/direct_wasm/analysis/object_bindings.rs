use super::*;

pub(in crate::backend::direct_wasm) fn enumerated_keys_from_object_binding(
    object_binding: &ObjectValueBinding,
) -> ArrayValueBinding {
    ArrayValueBinding {
        values: ordered_object_property_names(object_binding)
            .into_iter()
            .filter(|name| {
                !object_binding
                    .non_enumerable_string_properties
                    .iter()
                    .any(|hidden_name| hidden_name == name)
            })
            .map(|name| Some(Expression::String(name)))
            .collect(),
    }
}

pub(in crate::backend::direct_wasm) fn own_property_names_from_object_binding(
    object_binding: &ObjectValueBinding,
) -> ArrayValueBinding {
    ArrayValueBinding {
        values: ordered_object_property_names(object_binding)
            .into_iter()
            .map(|name| Some(Expression::String(name)))
            .collect(),
    }
}

pub(in crate::backend::direct_wasm) fn own_property_names_from_function_binding(
    object_binding: Option<&ObjectValueBinding>,
) -> ArrayValueBinding {
    let mut integer_keys = Vec::new();
    let mut other_keys = Vec::new();

    if let Some(object_binding) = object_binding {
        for name in ordered_object_property_names(object_binding) {
            if canonical_array_index_from_property_name(&name).is_some() {
                integer_keys.push(name);
            } else if name != "length" && name != "name" && name != "prototype" {
                other_keys.push(name);
            }
        }
    }

    let mut values = integer_keys
        .into_iter()
        .map(|name| Some(Expression::String(name)))
        .collect::<Vec<_>>();
    values.extend(
        ["length", "name", "prototype"]
            .into_iter()
            .map(|name| Some(Expression::String(name.to_string()))),
    );
    values.extend(
        other_keys
            .into_iter()
            .map(|name| Some(Expression::String(name))),
    );
    ArrayValueBinding { values }
}

pub(in crate::backend::direct_wasm) fn own_property_symbols_from_object_binding(
    object_binding: &ObjectValueBinding,
) -> ArrayValueBinding {
    ArrayValueBinding {
        values: object_binding
            .symbol_properties
            .iter()
            .map(|(key, _)| Some(key.clone()))
            .collect(),
    }
}

pub(in crate::backend::direct_wasm) fn own_property_names_from_array_binding(
    array_binding: &ArrayValueBinding,
) -> ArrayValueBinding {
    let mut values = array_binding
        .values
        .iter()
        .enumerate()
        .filter(|(_, value)| value.is_some())
        .map(|(index, _)| Some(Expression::String(index.to_string())))
        .collect::<Vec<_>>();
    values.push(Some(Expression::String("length".to_string())));
    ArrayValueBinding { values }
}

pub(in crate::backend::direct_wasm) fn ordered_object_property_names(
    object_binding: &ObjectValueBinding,
) -> Vec<String> {
    let mut integer_keys = Vec::new();
    let mut other_keys = Vec::new();

    for (name, _) in &object_binding.string_properties {
        if let Some(index) = canonical_array_index_from_property_name(name) {
            integer_keys.push((index, name.clone()));
        } else {
            other_keys.push(name.clone());
        }
    }

    integer_keys.sort_by_key(|(index, _)| *index);
    integer_keys
        .into_iter()
        .map(|(_, name)| name)
        .chain(other_keys)
        .collect()
}

pub(in crate::backend::direct_wasm) fn is_internal_user_function_identifier(name: &str) -> bool {
    name.starts_with("__ayy_")
}

pub(in crate::backend::direct_wasm) fn object_binding_set_string_property_enumerable(
    object_binding: &mut ObjectValueBinding,
    property_name: &str,
    enumerable: bool,
) {
    if enumerable {
        object_binding
            .non_enumerable_string_properties
            .retain(|hidden_name| hidden_name != property_name);
    } else if !object_binding
        .non_enumerable_string_properties
        .iter()
        .any(|hidden_name| hidden_name == property_name)
    {
        object_binding
            .non_enumerable_string_properties
            .push(property_name.to_string());
    }
}

pub(in crate::backend::direct_wasm) fn substitute_self_referential_binding_snapshot(
    expression: &Expression,
    name: &str,
    snapshot: &Expression,
) -> Expression {
    match expression {
        Expression::Identifier(identifier) if identifier == name => snapshot.clone(),
        Expression::Member { object, property } => Expression::Member {
            object: Box::new(substitute_self_referential_binding_snapshot(
                object, name, snapshot,
            )),
            property: Box::new(substitute_self_referential_binding_snapshot(
                property, name, snapshot,
            )),
        },
        Expression::Assign {
            name: target,
            value,
        } => Expression::Assign {
            name: target.clone(),
            value: Box::new(substitute_self_referential_binding_snapshot(
                value, name, snapshot,
            )),
        },
        Expression::AssignMember {
            object,
            property,
            value,
        } => Expression::AssignMember {
            object: Box::new(substitute_self_referential_binding_snapshot(
                object, name, snapshot,
            )),
            property: Box::new(substitute_self_referential_binding_snapshot(
                property, name, snapshot,
            )),
            value: Box::new(substitute_self_referential_binding_snapshot(
                value, name, snapshot,
            )),
        },
        Expression::AssignSuperMember { property, value } => Expression::AssignSuperMember {
            property: Box::new(substitute_self_referential_binding_snapshot(
                property, name, snapshot,
            )),
            value: Box::new(substitute_self_referential_binding_snapshot(
                value, name, snapshot,
            )),
        },
        Expression::Await(value) => Expression::Await(Box::new(
            substitute_self_referential_binding_snapshot(value, name, snapshot),
        )),
        Expression::EnumerateKeys(value) => Expression::EnumerateKeys(Box::new(
            substitute_self_referential_binding_snapshot(value, name, snapshot),
        )),
        Expression::GetIterator(value) => Expression::GetIterator(Box::new(
            substitute_self_referential_binding_snapshot(value, name, snapshot),
        )),
        Expression::IteratorClose(value) => Expression::IteratorClose(Box::new(
            substitute_self_referential_binding_snapshot(value, name, snapshot),
        )),
        Expression::Unary { op, expression } => Expression::Unary {
            op: *op,
            expression: Box::new(substitute_self_referential_binding_snapshot(
                expression, name, snapshot,
            )),
        },
        Expression::Binary { op, left, right } => Expression::Binary {
            op: *op,
            left: Box::new(substitute_self_referential_binding_snapshot(
                left, name, snapshot,
            )),
            right: Box::new(substitute_self_referential_binding_snapshot(
                right, name, snapshot,
            )),
        },
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => Expression::Conditional {
            condition: Box::new(substitute_self_referential_binding_snapshot(
                condition, name, snapshot,
            )),
            then_expression: Box::new(substitute_self_referential_binding_snapshot(
                then_expression,
                name,
                snapshot,
            )),
            else_expression: Box::new(substitute_self_referential_binding_snapshot(
                else_expression,
                name,
                snapshot,
            )),
        },
        Expression::Sequence(expressions) => Expression::Sequence(
            expressions
                .iter()
                .map(|expression| {
                    substitute_self_referential_binding_snapshot(expression, name, snapshot)
                })
                .collect(),
        ),
        Expression::Call { callee, arguments } => Expression::Call {
            callee: Box::new(substitute_self_referential_binding_snapshot(
                callee, name, snapshot,
            )),
            arguments: arguments
                .iter()
                .map(|argument| match argument {
                    CallArgument::Expression(expression) => CallArgument::Expression(
                        substitute_self_referential_binding_snapshot(expression, name, snapshot),
                    ),
                    CallArgument::Spread(expression) => CallArgument::Spread(
                        substitute_self_referential_binding_snapshot(expression, name, snapshot),
                    ),
                })
                .collect(),
        },
        Expression::SuperCall { callee, arguments } => Expression::SuperCall {
            callee: Box::new(substitute_self_referential_binding_snapshot(
                callee, name, snapshot,
            )),
            arguments: arguments
                .iter()
                .map(|argument| match argument {
                    CallArgument::Expression(expression) => CallArgument::Expression(
                        substitute_self_referential_binding_snapshot(expression, name, snapshot),
                    ),
                    CallArgument::Spread(expression) => CallArgument::Spread(
                        substitute_self_referential_binding_snapshot(expression, name, snapshot),
                    ),
                })
                .collect(),
        },
        Expression::New { callee, arguments } => Expression::New {
            callee: Box::new(substitute_self_referential_binding_snapshot(
                callee, name, snapshot,
            )),
            arguments: arguments
                .iter()
                .map(|argument| match argument {
                    CallArgument::Expression(expression) => CallArgument::Expression(
                        substitute_self_referential_binding_snapshot(expression, name, snapshot),
                    ),
                    CallArgument::Spread(expression) => CallArgument::Spread(
                        substitute_self_referential_binding_snapshot(expression, name, snapshot),
                    ),
                })
                .collect(),
        },
        Expression::Array(elements) => Expression::Array(
            elements
                .iter()
                .map(|element| match element {
                    crate::ir::hir::ArrayElement::Expression(expression) => {
                        crate::ir::hir::ArrayElement::Expression(
                            substitute_self_referential_binding_snapshot(
                                expression, name, snapshot,
                            ),
                        )
                    }
                    crate::ir::hir::ArrayElement::Spread(expression) => {
                        crate::ir::hir::ArrayElement::Spread(
                            substitute_self_referential_binding_snapshot(
                                expression, name, snapshot,
                            ),
                        )
                    }
                })
                .collect(),
        ),
        Expression::Object(entries) => Expression::Object(
            entries
                .iter()
                .map(|entry| match entry {
                    crate::ir::hir::ObjectEntry::Data { key, value } => {
                        crate::ir::hir::ObjectEntry::Data {
                            key: substitute_self_referential_binding_snapshot(key, name, snapshot),
                            value: substitute_self_referential_binding_snapshot(
                                value, name, snapshot,
                            ),
                        }
                    }
                    crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                        crate::ir::hir::ObjectEntry::Getter {
                            key: substitute_self_referential_binding_snapshot(key, name, snapshot),
                            getter: substitute_self_referential_binding_snapshot(
                                getter, name, snapshot,
                            ),
                        }
                    }
                    crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                        crate::ir::hir::ObjectEntry::Setter {
                            key: substitute_self_referential_binding_snapshot(key, name, snapshot),
                            setter: substitute_self_referential_binding_snapshot(
                                setter, name, snapshot,
                            ),
                        }
                    }
                    crate::ir::hir::ObjectEntry::Spread(expression) => {
                        crate::ir::hir::ObjectEntry::Spread(
                            substitute_self_referential_binding_snapshot(
                                expression, name, snapshot,
                            ),
                        )
                    }
                })
                .collect(),
        ),
        Expression::SuperMember { property } => Expression::SuperMember {
            property: Box::new(substitute_self_referential_binding_snapshot(
                property, name, snapshot,
            )),
        },
        _ => expression.clone(),
    }
}

pub(in crate::backend::direct_wasm) fn object_binding_lookup_value<'a>(
    object_binding: &'a ObjectValueBinding,
    property: &Expression,
) -> Option<&'a Expression> {
    if let Some(property_name) = static_property_name_from_expression(property) {
        return object_binding
            .string_properties
            .iter()
            .find(|(existing_name, _)| *existing_name == property_name)
            .map(|(_, value)| value);
    }
    object_binding
        .symbol_properties
        .iter()
        .find(|(existing_key, _)| existing_key == property)
        .map(|(_, value)| value)
}

pub(in crate::backend::direct_wasm) fn object_binding_has_property(
    object_binding: &ObjectValueBinding,
    property: &Expression,
) -> bool {
    object_binding_lookup_value(object_binding, property).is_some()
}

pub(in crate::backend::direct_wasm) fn object_binding_set_property(
    object_binding: &mut ObjectValueBinding,
    property: Expression,
    value: Expression,
) {
    if let Some(property_name) = static_property_name_from_expression(&property) {
        if let Some((_, existing_value)) = object_binding
            .string_properties
            .iter_mut()
            .find(|(existing_name, _)| *existing_name == property_name)
        {
            *existing_value = value;
        } else {
            object_binding
                .string_properties
                .push((property_name.clone(), value));
        }
        object_binding_set_string_property_enumerable(object_binding, &property_name, true);
        return;
    }

    if let Some((_, existing_value)) = object_binding
        .symbol_properties
        .iter_mut()
        .find(|(existing_key, _)| *existing_key == property)
    {
        *existing_value = value;
    } else {
        object_binding.symbol_properties.push((property, value));
    }
}

pub(in crate::backend::direct_wasm) fn object_binding_define_property(
    object_binding: &mut ObjectValueBinding,
    property: Expression,
    value: Expression,
    enumerable: bool,
) {
    if let Some(property_name) = static_property_name_from_expression(&property) {
        if let Some((_, existing_value)) = object_binding
            .string_properties
            .iter_mut()
            .find(|(existing_name, _)| *existing_name == property_name)
        {
            *existing_value = value;
        } else {
            object_binding
                .string_properties
                .push((property_name.clone(), value));
        }
        object_binding_set_string_property_enumerable(object_binding, &property_name, enumerable);
        return;
    }

    if let Some((_, existing_value)) = object_binding
        .symbol_properties
        .iter_mut()
        .find(|(existing_key, _)| *existing_key == property)
    {
        *existing_value = value;
    } else {
        object_binding.symbol_properties.push((property, value));
    }
}

pub(in crate::backend::direct_wasm) fn object_binding_remove_property(
    object_binding: &mut ObjectValueBinding,
    property: &Expression,
) -> bool {
    if let Some(property_name) = static_property_name_from_expression(property) {
        let len_before = object_binding.string_properties.len();
        object_binding
            .string_properties
            .retain(|(existing_name, _)| *existing_name != property_name);
        object_binding
            .non_enumerable_string_properties
            .retain(|hidden_name| hidden_name != &property_name);
        return object_binding.string_properties.len() != len_before;
    }

    let len_before = object_binding.symbol_properties.len();
    object_binding
        .symbol_properties
        .retain(|(existing_key, _)| existing_key != property);
    object_binding.symbol_properties.len() != len_before
}

pub(in crate::backend::direct_wasm) fn merge_enumerable_object_binding(
    target: &mut ObjectValueBinding,
    source: &ObjectValueBinding,
) {
    for (name, value) in &source.string_properties {
        if source
            .non_enumerable_string_properties
            .iter()
            .any(|hidden_name| hidden_name == name)
        {
            continue;
        }
        object_binding_set_property(target, Expression::String(name.clone()), value.clone());
    }
    for (property, value) in &source.symbol_properties {
        object_binding_set_property(target, property.clone(), value.clone());
    }
}

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
        | "Promise" | "eval" => Some(StaticValueKind::Function),
        "Math" | "JSON" | "globalThis" => Some(StaticValueKind::Object),
        "Infinity" | "NaN" | "undefined" => Some(StaticValueKind::Undefined),
        _ => None,
    }
}
