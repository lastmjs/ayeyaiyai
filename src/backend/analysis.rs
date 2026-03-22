fn infer_call_result_kind(name: &str) -> Option<StaticValueKind> {
    match name {
        "Number" => Some(StaticValueKind::Number),
        "String" => Some(StaticValueKind::String),
        "Boolean" => Some(StaticValueKind::Bool),
        "isNaN" => Some(StaticValueKind::Bool),
        "Object" | "Array" | "ArrayBuffer" | "Date" | "RegExp" | "Map" | "Set" | "Error"
        | "EvalError" | "RangeError" | "ReferenceError" | "SyntaxError" | "TypeError"
        | "URIError" | "AggregateError" | "Promise" => Some(StaticValueKind::Object),
        "Uint8Array" | "Int8Array" | "Uint16Array" | "Int16Array" | "Uint32Array"
        | "Int32Array" | "Float32Array" | "Float64Array" | "Uint8ClampedArray" => {
            Some(StaticValueKind::Object)
        }
        "BigInt" => Some(StaticValueKind::BigInt),
        "Symbol" => Some(StaticValueKind::Symbol),
        "Function" | FUNCTION_CONSTRUCTOR_FAMILY_BUILTIN | "eval" => {
            Some(StaticValueKind::Function)
        }
        _ => None,
    }
}

fn is_builtin_like_capture_identifier(name: &str) -> bool {
    name == "eval"
        || name == "console"
        || matches!(
            name,
            "__assert" | "__assertSameValue" | "__assertNotSameValue" | "__ayyAssertThrows"
        )
        || builtin_identifier_kind(name).is_some()
        || infer_call_result_kind(name).is_some()
}

fn function_constructor_literal_source_parts(
    arguments: &[CallArgument],
) -> Option<(String, String)> {
    let parts = arguments
        .iter()
        .map(|argument| match argument {
            CallArgument::Expression(Expression::String(text)) => Some(text.clone()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;

    let Some((body_source, parameter_sources)) = parts.split_last() else {
        return Some((String::new(), String::new()));
    };

    Some((parameter_sources.join(","), body_source.clone()))
}

fn function_constructor_wrapper_sources(
    name: &str,
    parameter_source: &str,
    body_source: &str,
) -> Option<Vec<String>> {
    let wrap = |prefix: &str| -> String {
        format!("{prefix} __ayy_ctor({parameter_source}) {{\n{body_source}\n}}")
    };

    match name {
        "Function" => Some(vec![wrap("function")]),
        "AsyncFunction" => Some(vec![wrap("async function")]),
        "GeneratorFunction" => Some(vec![wrap("function*")]),
        "AsyncGeneratorFunction" => Some(vec![wrap("async function*")]),
        FUNCTION_CONSTRUCTOR_FAMILY_BUILTIN => Some(vec![
            wrap("function"),
            wrap("async function"),
            wrap("function*"),
            wrap("async function*"),
        ]),
        _ => None,
    }
}

fn is_function_constructor_builtin(name: &str) -> bool {
    matches!(
        name,
        "Function"
            | "AsyncFunction"
            | "GeneratorFunction"
            | "AsyncGeneratorFunction"
            | FUNCTION_CONSTRUCTOR_FAMILY_BUILTIN
    )
}

fn are_function_constructor_bindings(bindings: &[LocalFunctionBinding]) -> bool {
    bindings.iter().all(|binding| {
        matches!(
            binding,
            LocalFunctionBinding::Builtin(name) if is_function_constructor_builtin(name)
        )
    })
}

fn typed_array_builtin_bytes_per_element(name: &str) -> Option<u32> {
    match name {
        "Uint8Array" | "Int8Array" | "Uint8ClampedArray" => Some(1),
        "Uint16Array" | "Int16Array" => Some(2),
        "Uint32Array" | "Int32Array" | "Float32Array" => Some(4),
        "Float64Array" => Some(8),
        _ => None,
    }
}

fn is_bytes_per_element_access(expression: &Expression) -> bool {
    matches!(
        expression,
        Expression::Member { property, .. }
            if matches!(property.as_ref(), Expression::String(name) if name == "BYTES_PER_ELEMENT")
    )
}

fn extract_typed_array_element_count(expression: &Expression) -> Option<usize> {
    match expression {
        Expression::Number(value) if value.is_finite() && value.fract() == 0.0 && *value >= 0.0 => {
            Some(*value as usize)
        }
        Expression::Binary {
            op: BinaryOp::Multiply,
            left,
            right,
        } => match (left.as_ref(), right.as_ref()) {
            (Expression::Number(multiplier), factor) | (factor, Expression::Number(multiplier))
                if multiplier.is_finite()
                    && multiplier.fract() == 0.0
                    && *multiplier >= 0.0
                    && is_bytes_per_element_access(factor) =>
            {
                Some(*multiplier as usize)
            }
            _ => None,
        },
        _ => None,
    }
}

fn native_error_runtime_value(name: &str) -> Option<i32> {
    NATIVE_ERROR_NAMES
        .iter()
        .position(|candidate| *candidate == name)
        .map(|offset| JS_NATIVE_ERROR_VALUE_BASE + offset as i32)
}

fn native_error_instanceof_values(name: &str) -> Option<Vec<i32>> {
    if name == "Error" {
        return Some(
            NATIVE_ERROR_NAMES
                .iter()
                .filter_map(|candidate| native_error_runtime_value(candidate))
                .collect(),
        );
    }

    native_error_runtime_value(name).map(|value| vec![value])
}

fn is_arguments_identifier(expression: &Expression) -> bool {
    matches!(expression, Expression::Identifier(name) if name == "arguments")
}

fn is_symbol_iterator_expression(expression: &Expression) -> bool {
    matches!(
        expression,
        Expression::Member { object, property }
            if matches!(object.as_ref(), Expression::Identifier(name) if name == "Symbol")
                && matches!(property.as_ref(), Expression::String(name) if name == "iterator")
    )
}

fn symbol_iterator_expression() -> Expression {
    Expression::Member {
        object: Box::new(Expression::Identifier("Symbol".to_string())),
        property: Box::new(Expression::String("iterator".to_string())),
    }
}

fn arguments_symbol_iterator_expression() -> Expression {
    Expression::Member {
        object: Box::new(Expression::Array(Vec::new())),
        property: Box::new(symbol_iterator_expression()),
    }
}

fn symbol_to_primitive_expression() -> Expression {
    Expression::Member {
        object: Box::new(Expression::Identifier("Symbol".to_string())),
        property: Box::new(Expression::String("toPrimitive".to_string())),
    }
}

fn argument_index_from_expression(expression: &Expression) -> Option<u32> {
    match expression {
        Expression::Number(value) if value.is_finite() && value.fract() == 0.0 && *value >= 0.0 => {
            let index = *value as u64;
            (index <= u32::MAX as u64).then_some(index as u32)
        }
        Expression::String(text) => canonical_array_index_from_property_name(text),
        _ => None,
    }
}

fn canonical_array_index_from_property_name(text: &str) -> Option<u32> {
    let index = text.parse::<u32>().ok()?;
    if index == u32::MAX || index.to_string() != text {
        return None;
    }
    Some(index)
}

fn normalize_js_scientific_notation(text: String) -> String {
    let Some((mantissa, exponent)) = text.split_once('e') else {
        return text;
    };
    let Ok(exponent_value) = exponent.parse::<i32>() else {
        return text;
    };
    if exponent_value >= 0 {
        format!("{mantissa}e+{exponent_value}")
    } else {
        format!("{mantissa}e{exponent_value}")
    }
}

fn js_number_property_name(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value == 0.0 {
        return "0".to_string();
    }
    if value == f64::INFINITY {
        return "Infinity".to_string();
    }
    if value == f64::NEG_INFINITY {
        return "-Infinity".to_string();
    }

    let abs = value.abs();
    if abs >= 1e21 || abs < 1e-6 {
        return normalize_js_scientific_notation(format!("{value:e}"));
    }

    value.to_string()
}

fn static_numeric_property_name_value(expression: &Expression) -> Option<f64> {
    match expression {
        Expression::Number(value) => Some(*value),
        Expression::Unary {
            op: UnaryOp::Plus,
            expression,
        } => static_numeric_property_name_value(expression),
        Expression::Unary {
            op: UnaryOp::Negate,
            expression,
        } => Some(-static_numeric_property_name_value(expression)?),
        _ => None,
    }
}

fn static_property_name_from_expression(expression: &Expression) -> Option<String> {
    match expression {
        Expression::String(text) => Some(text.clone()),
        Expression::Bool(value) => Some(value.to_string()),
        Expression::BigInt(value) => Some(value.clone()),
        Expression::Null => Some("null".to_string()),
        Expression::Undefined => Some("undefined".to_string()),
        _ => static_numeric_property_name_value(expression).map(js_number_property_name),
    }
}

fn hex_digit_value(character: char) -> Option<u32> {
    match character {
        '0'..='9' => Some(character as u32 - '0' as u32),
        'A'..='F' => Some(character as u32 - 'A' as u32 + 10),
        'a'..='f' => Some(character as u32 - 'a' as u32 + 10),
        _ => None,
    }
}

fn parse_fixed_hex_quad(text: &str) -> Option<u32> {
    if text.len() != 4 {
        return None;
    }

    let mut value = 0u32;
    for character in text.chars() {
        value = (value << 4) | hex_digit_value(character)?;
    }
    Some(value)
}

fn is_canonical_hex_digit_array(array_binding: &ArrayValueBinding) -> bool {
    const DIGITS: [&str; 16] = [
        "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "A", "B", "C", "D", "E", "F",
    ];

    array_binding.values.len() == DIGITS.len()
        && array_binding
            .values
            .iter()
            .zip(DIGITS.iter())
            .all(|(value, digit)| matches!(value, Some(Expression::String(text)) if text == digit))
}

fn enumerated_keys_from_array_binding(array_binding: &ArrayValueBinding) -> ArrayValueBinding {
    ArrayValueBinding {
        values: array_binding
            .values
            .iter()
            .enumerate()
            .filter(|(_, value)| value.is_some())
            .map(|(index, _)| Some(Expression::String(index.to_string())))
            .collect(),
    }
}

fn enumerated_keys_from_object_binding(object_binding: &ObjectValueBinding) -> ArrayValueBinding {
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

fn own_property_names_from_object_binding(
    object_binding: &ObjectValueBinding,
) -> ArrayValueBinding {
    ArrayValueBinding {
        values: ordered_object_property_names(object_binding)
            .into_iter()
            .map(|name| Some(Expression::String(name)))
            .collect(),
    }
}

fn own_property_names_from_function_binding(
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

fn own_property_symbols_from_object_binding(
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

fn own_property_names_from_array_binding(array_binding: &ArrayValueBinding) -> ArrayValueBinding {
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

fn ordered_object_property_names(object_binding: &ObjectValueBinding) -> Vec<String> {
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

fn is_internal_user_function_identifier(name: &str) -> bool {
    name.starts_with("__ayy_")
}

fn object_binding_set_string_property_enumerable(
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

fn substitute_self_referential_binding_snapshot(
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
                    crate::ir::hir::ObjectEntry::Data { key, value } => crate::ir::hir::ObjectEntry::Data {
                        key: substitute_self_referential_binding_snapshot(key, name, snapshot),
                        value: substitute_self_referential_binding_snapshot(value, name, snapshot),
                    },
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
                    crate::ir::hir::ObjectEntry::Spread(expression) => crate::ir::hir::ObjectEntry::Spread(
                        substitute_self_referential_binding_snapshot(expression, name, snapshot),
                    ),
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

fn object_binding_lookup_value<'a>(
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

fn object_binding_has_property(object_binding: &ObjectValueBinding, property: &Expression) -> bool {
    object_binding_lookup_value(object_binding, property).is_some()
}

fn object_binding_set_property(
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

fn object_binding_define_property(
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

fn object_binding_remove_property(
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

fn merge_enumerable_object_binding(target: &mut ObjectValueBinding, source: &ObjectValueBinding) {
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

fn object_binding_to_expression(object_binding: &ObjectValueBinding) -> Expression {
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

fn object_literal_prototype_expression(expression: &Expression) -> Option<Expression> {
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

fn resolve_property_descriptor_definition(
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

fn infer_global_expression_kind(expression: &Expression) -> StaticValueKind {
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

fn expand_static_array_binding(
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

fn expand_static_call_arguments(
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

fn builtin_identifier_kind(name: &str) -> Option<StaticValueKind> {
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

fn collect_arguments_usage_from_statements(statements: &[Statement]) -> ArgumentsUsage {
    let mut indexed_slots = HashSet::new();
    let mut track_all_slots = false;
    for statement in statements {
        collect_arguments_usage_from_statement(statement, &mut indexed_slots, &mut track_all_slots);
    }
    if track_all_slots {
        indexed_slots.extend(0..TRACKED_ARGUMENT_SLOT_LIMIT);
    }
    let mut indexed_slots = indexed_slots.into_iter().collect::<Vec<_>>();
    indexed_slots.sort_unstable();
    ArgumentsUsage { indexed_slots }
}

fn function_returns_arguments_object(statements: &[Statement]) -> bool {
    statements.iter().any(statement_returns_arguments_object)
}

fn collect_returned_arguments_effects(statements: &[Statement]) -> ReturnedArgumentsEffects {
    let mut effects = ReturnedArgumentsEffects::default();
    for statement in statements {
        collect_returned_arguments_effects_from_statement(statement, &mut effects);
    }
    effects
}

fn statement_returns_arguments_object(statement: &Statement) -> bool {
    match statement {
        Statement::Return(Expression::Identifier(name)) => name == "arguments",
        Statement::Block { body } | Statement::Labeled { body, .. } => {
            body.iter().any(statement_returns_arguments_object)
        }
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => {
            then_branch.iter().any(statement_returns_arguments_object)
                || else_branch.iter().any(statement_returns_arguments_object)
        }
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => {
            body.iter().any(statement_returns_arguments_object)
                || catch_setup.iter().any(statement_returns_arguments_object)
                || catch_body.iter().any(statement_returns_arguments_object)
        }
        Statement::Switch { cases, .. } => cases
            .iter()
            .any(|case| case.body.iter().any(statement_returns_arguments_object)),
        Statement::For { init, body, .. } => {
            init.iter().any(statement_returns_arguments_object)
                || body.iter().any(statement_returns_arguments_object)
        }
        Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
            body.iter().any(statement_returns_arguments_object)
        }
        _ => false,
    }
}

fn collect_returned_arguments_effects_from_statement(
    statement: &Statement,
    effects: &mut ReturnedArgumentsEffects,
) {
    match statement {
        Statement::Block { body } | Statement::Labeled { body, .. } => {
            for statement in body {
                collect_returned_arguments_effects_from_statement(statement, effects);
            }
        }
        Statement::Var { value, .. }
        | Statement::Let { value, .. }
        | Statement::Assign { value, .. }
        | Statement::Expression(value)
        | Statement::Throw(value)
        | Statement::Return(value)
        | Statement::Yield { value }
        | Statement::YieldDelegate { value } => {
            collect_returned_arguments_effects_from_expression(value, effects);
        }
        Statement::AssignMember {
            object,
            property,
            value,
        } => {
            collect_returned_arguments_effects_from_expression(value, effects);
            if let Some(property_name) = direct_arguments_named_property(object, property) {
                let effect = ArgumentsPropertyEffect::Assign(value.clone());
                match property_name {
                    "callee" => effects.callee = Some(effect),
                    "length" => effects.length = Some(effect),
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn collect_returned_arguments_effects_from_expression(
    expression: &Expression,
    effects: &mut ReturnedArgumentsEffects,
) {
    match expression {
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            collect_returned_arguments_effects_from_expression(value, effects);
            if let Some(property_name) = direct_arguments_named_property(object, property) {
                let effect = ArgumentsPropertyEffect::Assign((**value).clone());
                match property_name {
                    "callee" => effects.callee = Some(effect),
                    "length" => effects.length = Some(effect),
                    _ => {}
                }
            }
        }
        Expression::Unary {
            op: UnaryOp::Delete,
            expression,
        } => {
            if let Expression::Member { object, property } = expression.as_ref() {
                if let Some(property_name) = direct_arguments_named_property(object, property) {
                    match property_name {
                        "callee" => effects.callee = Some(ArgumentsPropertyEffect::Delete),
                        "length" => effects.length = Some(ArgumentsPropertyEffect::Delete),
                        _ => {}
                    }
                }
            }
        }
        Expression::Sequence(expressions) => {
            for expression in expressions {
                collect_returned_arguments_effects_from_expression(expression, effects);
            }
        }
        Expression::Binary { left, right, .. } => {
            collect_returned_arguments_effects_from_expression(left, effects);
            collect_returned_arguments_effects_from_expression(right, effects);
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            collect_returned_arguments_effects_from_expression(condition, effects);
            collect_returned_arguments_effects_from_expression(then_expression, effects);
            collect_returned_arguments_effects_from_expression(else_expression, effects);
        }
        Expression::Call { callee, arguments }
        | Expression::SuperCall { callee, arguments }
        | Expression::New { callee, arguments } => {
            collect_returned_arguments_effects_from_expression(callee, effects);
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        collect_returned_arguments_effects_from_expression(expression, effects);
                    }
                }
            }
        }
        Expression::Member { object, property } => {
            collect_returned_arguments_effects_from_expression(object, effects);
            collect_returned_arguments_effects_from_expression(property, effects);
        }
        Expression::Assign { value, .. }
        | Expression::AssignSuperMember { value, .. }
        | Expression::Await(value)
        | Expression::EnumerateKeys(value)
        | Expression::GetIterator(value)
        | Expression::IteratorClose(value)
        | Expression::Unary {
            expression: value, ..
        } => {
            collect_returned_arguments_effects_from_expression(value, effects);
        }
        Expression::SuperMember { property } => {
            collect_returned_arguments_effects_from_expression(property, effects);
        }
        _ => {}
    }
}

fn direct_arguments_named_property(
    object: &Expression,
    property: &Expression,
) -> Option<&'static str> {
    if !is_arguments_identifier(object) {
        return None;
    }
    match property {
        Expression::String(property_name) if property_name == "callee" => Some("callee"),
        Expression::String(property_name) if property_name == "length" => Some("length"),
        _ => None,
    }
}

fn collect_returned_member_function_bindings(
    statements: &[Statement],
    function_names: &HashSet<String>,
) -> Vec<ReturnedMemberFunctionBinding> {
    let Some(returned_identifier) = collect_returned_identifier(statements) else {
        return Vec::new();
    };

    let mut bindings = HashMap::new();
    for statement in statements {
        collect_returned_member_function_bindings_from_statement(
            statement,
            &returned_identifier,
            function_names,
            &mut bindings,
        );
    }

    bindings
        .into_iter()
        .map(|(key, binding)| ReturnedMemberFunctionBinding {
            target: key.target,
            property: key.property,
            binding,
        })
        .collect()
}

fn collect_returned_identifier(statements: &[Statement]) -> Option<String> {
    statements
        .iter()
        .rev()
        .find_map(collect_returned_identifier_from_statement)
}

fn collect_returned_identifier_source_expression(statements: &[Statement]) -> Option<Expression> {
    let returned_identifier = collect_returned_identifier(statements)?;
    statements.iter().rev().find_map(|statement| {
        collect_returned_identifier_source_expression_from_statement(
            statement,
            &returned_identifier,
        )
    })
}

fn collect_returned_identifier_from_statement(statement: &Statement) -> Option<String> {
    match statement {
        Statement::Return(Expression::Identifier(name)) => Some(name.clone()),
        Statement::Block { body }
        | Statement::Labeled { body, .. }
        | Statement::With { body, .. } => collect_returned_identifier(body),
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => collect_returned_identifier(then_branch)
            .or_else(|| collect_returned_identifier(else_branch)),
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => collect_returned_identifier(body)
            .or_else(|| collect_returned_identifier(catch_setup))
            .or_else(|| collect_returned_identifier(catch_body)),
        Statement::Switch { cases, .. } => cases
            .iter()
            .rev()
            .find_map(|case| collect_returned_identifier(&case.body)),
        Statement::For { init, body, .. } => {
            collect_returned_identifier(body).or_else(|| collect_returned_identifier(init))
        }
        Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
            collect_returned_identifier(body)
        }
        _ => None,
    }
}

fn collect_returned_identifier_source_expression_from_statement(
    statement: &Statement,
    returned_identifier: &str,
) -> Option<Expression> {
    match statement {
        Statement::Var { name, value }
        | Statement::Let { name, value, .. }
        | Statement::Assign { name, value }
            if name == returned_identifier =>
        {
            Some(value.clone())
        }
        Statement::Block { body }
        | Statement::Labeled { body, .. }
        | Statement::With { body, .. } => collect_returned_identifier_source_expression(body),
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => collect_returned_identifier_source_expression(then_branch)
            .or_else(|| collect_returned_identifier_source_expression(else_branch)),
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => collect_returned_identifier_source_expression(body)
            .or_else(|| collect_returned_identifier_source_expression(catch_setup))
            .or_else(|| collect_returned_identifier_source_expression(catch_body)),
        Statement::Switch { cases, .. } => cases
            .iter()
            .rev()
            .find_map(|case| collect_returned_identifier_source_expression(&case.body)),
        Statement::For { init, body, .. } => collect_returned_identifier_source_expression(body)
            .or_else(|| collect_returned_identifier_source_expression(init)),
        Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
            collect_returned_identifier_source_expression(body)
        }
        _ => None,
    }
}

fn collect_returned_member_value_bindings(
    statements: &[Statement],
) -> Vec<ReturnedMemberValueBinding> {
    if let Some(entries) = collect_returned_object_literal(statements) {
        return entries
            .into_iter()
            .filter_map(|entry| match entry {
                crate::ir::hir::ObjectEntry::Data {
                    key: Expression::String(property),
                    value,
                } => Some(ReturnedMemberValueBinding { property, value }),
                _ => None,
            })
            .collect();
    }

    let Some(returned_identifier) = collect_returned_identifier(statements) else {
        return Vec::new();
    };
    let local_aliases = collect_returned_member_local_aliases(statements);

    let mut bindings = HashMap::new();
    for statement in statements {
        collect_returned_member_value_bindings_from_statement(
            statement,
            &returned_identifier,
            &local_aliases,
            &mut bindings,
        );
    }

    bindings
        .into_iter()
        .map(|(property, value)| ReturnedMemberValueBinding { property, value })
        .collect()
}

fn collect_returned_member_local_aliases(statements: &[Statement]) -> HashMap<String, Expression> {
    let mut aliases = HashMap::new();
    for statement in statements {
        collect_returned_member_local_aliases_from_statement(statement, &mut aliases);
    }
    aliases
}

fn collect_returned_member_local_aliases_from_statement(
    statement: &Statement,
    aliases: &mut HashMap<String, Expression>,
) {
    match statement {
        Statement::Block { body }
        | Statement::Labeled { body, .. }
        | Statement::With { body, .. } => {
            for statement in body {
                collect_returned_member_local_aliases_from_statement(statement, aliases);
            }
        }
        Statement::Var { name, value } | Statement::Let { name, value, .. } => {
            aliases.insert(
                name.clone(),
                resolve_returned_member_local_alias_expression(value, aliases),
            );
        }
        Statement::Assign { name, value } => {
            aliases.insert(
                name.clone(),
                resolve_returned_member_local_alias_expression(value, aliases),
            );
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_returned_member_local_aliases_from_expression(condition, aliases);
            for statement in then_branch {
                collect_returned_member_local_aliases_from_statement(statement, aliases);
            }
            for statement in else_branch {
                collect_returned_member_local_aliases_from_statement(statement, aliases);
            }
        }
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => {
            for statement in body {
                collect_returned_member_local_aliases_from_statement(statement, aliases);
            }
            for statement in catch_setup {
                collect_returned_member_local_aliases_from_statement(statement, aliases);
            }
            for statement in catch_body {
                collect_returned_member_local_aliases_from_statement(statement, aliases);
            }
        }
        Statement::Switch {
            discriminant,
            cases,
            ..
        } => {
            collect_returned_member_local_aliases_from_expression(discriminant, aliases);
            for case in cases {
                if let Some(test) = &case.test {
                    collect_returned_member_local_aliases_from_expression(test, aliases);
                }
                for statement in &case.body {
                    collect_returned_member_local_aliases_from_statement(statement, aliases);
                }
            }
        }
        Statement::For {
            init,
            condition,
            update,
            break_hook,
            body,
            ..
        } => {
            for statement in init {
                collect_returned_member_local_aliases_from_statement(statement, aliases);
            }
            if let Some(condition) = condition {
                collect_returned_member_local_aliases_from_expression(condition, aliases);
            }
            if let Some(update) = update {
                collect_returned_member_local_aliases_from_expression(update, aliases);
            }
            if let Some(break_hook) = break_hook {
                collect_returned_member_local_aliases_from_expression(break_hook, aliases);
            }
            for statement in body {
                collect_returned_member_local_aliases_from_statement(statement, aliases);
            }
        }
        Statement::While {
            condition,
            break_hook,
            body,
            ..
        }
        | Statement::DoWhile {
            condition,
            break_hook,
            body,
            ..
        } => {
            collect_returned_member_local_aliases_from_expression(condition, aliases);
            if let Some(break_hook) = break_hook {
                collect_returned_member_local_aliases_from_expression(break_hook, aliases);
            }
            for statement in body {
                collect_returned_member_local_aliases_from_statement(statement, aliases);
            }
        }
        Statement::Expression(expression)
        | Statement::Throw(expression)
        | Statement::Return(expression)
        | Statement::Yield { value: expression }
        | Statement::YieldDelegate { value: expression } => {
            collect_returned_member_local_aliases_from_expression(expression, aliases);
        }
        Statement::Print { values } => {
            for value in values {
                collect_returned_member_local_aliases_from_expression(value, aliases);
            }
        }
        Statement::AssignMember {
            object,
            property,
            value,
        } => {
            collect_returned_member_local_aliases_from_expression(object, aliases);
            collect_returned_member_local_aliases_from_expression(property, aliases);
            collect_returned_member_local_aliases_from_expression(value, aliases);
        }
        Statement::Break { .. } | Statement::Continue { .. } => {}
    }
}

fn collect_returned_member_local_aliases_from_expression(
    expression: &Expression,
    aliases: &mut HashMap<String, Expression>,
) {
    match expression {
        Expression::Call { callee, arguments }
        | Expression::SuperCall { callee, arguments }
        | Expression::New { callee, arguments } => {
            collect_returned_member_local_aliases_from_expression(callee, aliases);
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        collect_returned_member_local_aliases_from_expression(expression, aliases);
                    }
                }
            }
        }
        Expression::Unary { expression, .. }
        | Expression::Await(expression)
        | Expression::EnumerateKeys(expression)
        | Expression::GetIterator(expression)
        | Expression::IteratorClose(expression) => {
            collect_returned_member_local_aliases_from_expression(expression, aliases);
        }
        Expression::Binary { left, right, .. } => {
            collect_returned_member_local_aliases_from_expression(left, aliases);
            collect_returned_member_local_aliases_from_expression(right, aliases);
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            collect_returned_member_local_aliases_from_expression(condition, aliases);
            collect_returned_member_local_aliases_from_expression(then_expression, aliases);
            collect_returned_member_local_aliases_from_expression(else_expression, aliases);
        }
        Expression::Sequence(expressions) => {
            for expression in expressions {
                collect_returned_member_local_aliases_from_expression(expression, aliases);
            }
        }
        Expression::Member { object, property } => {
            collect_returned_member_local_aliases_from_expression(object, aliases);
            collect_returned_member_local_aliases_from_expression(property, aliases);
        }
        Expression::Assign { value, .. } | Expression::AssignSuperMember { value, .. } => {
            collect_returned_member_local_aliases_from_expression(value, aliases);
        }
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            collect_returned_member_local_aliases_from_expression(object, aliases);
            collect_returned_member_local_aliases_from_expression(property, aliases);
            collect_returned_member_local_aliases_from_expression(value, aliases);
        }
        Expression::SuperMember { property } => {
            collect_returned_member_local_aliases_from_expression(property, aliases);
        }
        Expression::Array(elements) => {
            for element in elements {
                match element {
                    ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                        collect_returned_member_local_aliases_from_expression(expression, aliases);
                    }
                }
            }
        }
        Expression::Object(entries) => {
            for entry in entries {
                match entry {
                    ObjectEntry::Data { key, value } => {
                        collect_returned_member_local_aliases_from_expression(key, aliases);
                        collect_returned_member_local_aliases_from_expression(value, aliases);
                    }
                    ObjectEntry::Getter { key, getter } => {
                        collect_returned_member_local_aliases_from_expression(key, aliases);
                        collect_returned_member_local_aliases_from_expression(getter, aliases);
                    }
                    ObjectEntry::Setter { key, setter } => {
                        collect_returned_member_local_aliases_from_expression(key, aliases);
                        collect_returned_member_local_aliases_from_expression(setter, aliases);
                    }
                    ObjectEntry::Spread(value) => {
                        collect_returned_member_local_aliases_from_expression(value, aliases);
                    }
                }
            }
        }
        Expression::Number(_)
        | Expression::BigInt(_)
        | Expression::String(_)
        | Expression::Bool(_)
        | Expression::Null
        | Expression::Undefined
        | Expression::NewTarget
        | Expression::Identifier(_)
        | Expression::This
        | Expression::Sent
        | Expression::Update { .. } => {}
    }
}

fn resolve_returned_member_local_alias_expression(
    expression: &Expression,
    aliases: &HashMap<String, Expression>,
) -> Expression {
    let mut current = expression;
    let mut visited = HashSet::new();
    loop {
        let Expression::Identifier(name) = current else {
            return current.clone();
        };
        if !visited.insert(name.clone()) {
            return expression.clone();
        }
        let Some(next) = aliases.get(name) else {
            return current.clone();
        };
        current = next;
    }
}

fn collect_returned_object_literal(statements: &[Statement]) -> Option<Vec<ObjectEntry>> {
    statements
        .iter()
        .rev()
        .find_map(collect_returned_object_literal_from_statement)
}

fn collect_returned_object_literal_from_statement(
    statement: &Statement,
) -> Option<Vec<ObjectEntry>> {
    match statement {
        Statement::Return(Expression::Object(entries)) => Some(entries.clone()),
        Statement::Block { body }
        | Statement::Labeled { body, .. }
        | Statement::With { body, .. } => collect_returned_object_literal(body),
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => collect_returned_object_literal(then_branch)
            .or_else(|| collect_returned_object_literal(else_branch)),
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => collect_returned_object_literal(body)
            .or_else(|| collect_returned_object_literal(catch_setup))
            .or_else(|| collect_returned_object_literal(catch_body)),
        Statement::Switch { cases, .. } => cases
            .iter()
            .rev()
            .find_map(|case| collect_returned_object_literal(&case.body)),
        Statement::For { init, body, .. } => {
            collect_returned_object_literal(body).or_else(|| collect_returned_object_literal(init))
        }
        Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
            collect_returned_object_literal(body)
        }
        _ => None,
    }
}

fn collect_enumerated_keys_param_index(function: &FunctionDeclaration) -> Option<usize> {
    let returned_identifier = collect_returned_identifier(&function.body)?;
    let initialized_array = function.body.iter().any(|statement| {
        matches!(
            statement,
            Statement::Var { name, value }
                | Statement::Let { name, value, .. }
                | Statement::Assign { name, value }
                if name == &returned_identifier
                    && matches!(value, Expression::Array(elements) if elements.is_empty())
        )
    });
    if !initialized_array {
        return None;
    }

    function.body.iter().find_map(|statement| {
        match_enumerated_keys_collector_loop(statement, &returned_identifier, function)
    })
}

fn match_enumerated_keys_collector_loop(
    statement: &Statement,
    returned_identifier: &str,
    function: &FunctionDeclaration,
) -> Option<usize> {
    let Statement::For {
        init,
        condition,
        update,
        body,
        ..
    } = statement
    else {
        return None;
    };

    let (target_name, param_index) = init.iter().find_map(|statement| match statement {
        Statement::Let { name, value, .. } | Statement::Var { name, value } => {
            let Expression::Identifier(param_name) = value else {
                return None;
            };
            function
                .params
                .iter()
                .position(|parameter| parameter.name == *param_name)
                .map(|param_index| (name.clone(), param_index))
        }
        _ => None,
    })?;

    let keys_name = init.iter().find_map(|statement| match statement {
        Statement::Let { name, value, .. } | Statement::Var { name, value } => {
            let Expression::EnumerateKeys(target) = value else {
                return None;
            };
            matches!(target.as_ref(), Expression::Identifier(current_target) if current_target == &target_name)
                .then(|| name.clone())
        }
        _ => None,
    })?;

    let index_name = match condition.as_ref()? {
        Expression::Binary {
            op: BinaryOp::LessThan,
            left,
            right,
        } => {
            let Expression::Identifier(index_name) = left.as_ref() else {
                return None;
            };
            matches!(
                right.as_ref(),
                Expression::Member { object, property }
                    if matches!(object.as_ref(), Expression::Identifier(name) if name == &keys_name)
                        && matches!(property.as_ref(), Expression::String(property_name) if property_name == "length")
            )
            .then(|| index_name.clone())?
        }
        _ => return None,
    };

    if !matches!(
        update.as_ref()?,
        Expression::Update {
            name,
            op: UpdateOp::Increment,
            ..
        } if name == &index_name
    ) {
        return None;
    }

    let loop_value_name = body.iter().find_map(|statement| match statement {
        Statement::Let { name, value, .. } | Statement::Var { name, value } => matches!(
            value,
            Expression::Member { object, property }
                if matches!(object.as_ref(), Expression::Identifier(current_keys) if current_keys == &keys_name)
                    && matches!(property.as_ref(), Expression::Identifier(current_index) if current_index == &index_name)
        )
        .then(|| name.clone()),
        _ => None,
    })?;

    body.iter().any(|statement| {
        matches!(
            statement,
            Statement::Expression(Expression::Call { callee, arguments })
                if matches!(
                    callee.as_ref(),
                    Expression::Member { object, property }
                        if matches!(object.as_ref(), Expression::Identifier(name) if name == returned_identifier)
                            && matches!(property.as_ref(), Expression::String(property_name) if property_name == "push")
                ) && matches!(
                    arguments.as_slice(),
                    [CallArgument::Expression(Expression::Identifier(argument_name))]
                        if argument_name == &loop_value_name
                )
        )
    })
    .then_some(param_index)
}

fn collect_returned_member_value_bindings_from_statement(
    statement: &Statement,
    returned_identifier: &str,
    local_aliases: &HashMap<String, Expression>,
    bindings: &mut HashMap<String, Expression>,
) {
    match statement {
        Statement::Block { body }
        | Statement::Labeled { body, .. }
        | Statement::With { body, .. } => {
            for statement in body {
                collect_returned_member_value_bindings_from_statement(
                    statement,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
        }
        Statement::AssignMember {
            object,
            property,
            value,
        } => {
            if matches!(object, Expression::Identifier(name) if name == returned_identifier) {
                if let Expression::String(property_name) = property {
                    bindings.insert(property_name.clone(), value.clone());
                }
            }
            collect_returned_member_value_bindings_from_expression(
                object,
                returned_identifier,
                local_aliases,
                bindings,
            );
            collect_returned_member_value_bindings_from_expression(
                property,
                returned_identifier,
                local_aliases,
                bindings,
            );
            collect_returned_member_value_bindings_from_expression(
                value,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Statement::Var { value, .. }
        | Statement::Let { value, .. }
        | Statement::Assign { value, .. }
        | Statement::Expression(value)
        | Statement::Throw(value)
        | Statement::Return(value)
        | Statement::Yield { value }
        | Statement::YieldDelegate { value } => {
            collect_returned_member_value_bindings_from_expression(
                value,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Statement::Print { values } => {
            for value in values {
                collect_returned_member_value_bindings_from_expression(
                    value,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_returned_member_value_bindings_from_expression(
                condition,
                returned_identifier,
                local_aliases,
                bindings,
            );
            for statement in then_branch {
                collect_returned_member_value_bindings_from_statement(
                    statement,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
            for statement in else_branch {
                collect_returned_member_value_bindings_from_statement(
                    statement,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
        }
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => {
            for statement in body {
                collect_returned_member_value_bindings_from_statement(
                    statement,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
            for statement in catch_setup {
                collect_returned_member_value_bindings_from_statement(
                    statement,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
            for statement in catch_body {
                collect_returned_member_value_bindings_from_statement(
                    statement,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
        }
        Statement::Switch {
            discriminant,
            cases,
            ..
        } => {
            collect_returned_member_value_bindings_from_expression(
                discriminant,
                returned_identifier,
                local_aliases,
                bindings,
            );
            for case in cases {
                if let Some(test) = &case.test {
                    collect_returned_member_value_bindings_from_expression(
                        test,
                        returned_identifier,
                        local_aliases,
                        bindings,
                    );
                }
                for statement in &case.body {
                    collect_returned_member_value_bindings_from_statement(
                        statement,
                        returned_identifier,
                        local_aliases,
                        bindings,
                    );
                }
            }
        }
        Statement::For {
            init,
            condition,
            update,
            break_hook,
            body,
            ..
        } => {
            for statement in init {
                collect_returned_member_value_bindings_from_statement(
                    statement,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
            if let Some(condition) = condition {
                collect_returned_member_value_bindings_from_expression(
                    condition,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
            if let Some(update) = update {
                collect_returned_member_value_bindings_from_expression(
                    update,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
            if let Some(break_hook) = break_hook {
                collect_returned_member_value_bindings_from_expression(
                    break_hook,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
            for statement in body {
                collect_returned_member_value_bindings_from_statement(
                    statement,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
        }
        Statement::While {
            condition,
            break_hook,
            body,
            ..
        }
        | Statement::DoWhile {
            condition,
            break_hook,
            body,
            ..
        } => {
            collect_returned_member_value_bindings_from_expression(
                condition,
                returned_identifier,
                local_aliases,
                bindings,
            );
            if let Some(break_hook) = break_hook {
                collect_returned_member_value_bindings_from_expression(
                    break_hook,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
            for statement in body {
                collect_returned_member_value_bindings_from_statement(
                    statement,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
        }
        _ => {}
    }
}

fn collect_returned_member_value_bindings_from_expression(
    expression: &Expression,
    returned_identifier: &str,
    local_aliases: &HashMap<String, Expression>,
    bindings: &mut HashMap<String, Expression>,
) {
    match expression {
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            if matches!(object.as_ref(), Expression::Identifier(name) if name == returned_identifier)
            {
                if let Expression::String(property_name) = property.as_ref() {
                    bindings.insert(property_name.clone(), (**value).clone());
                }
            }
            collect_returned_member_value_bindings_from_expression(
                object,
                returned_identifier,
                local_aliases,
                bindings,
            );
            collect_returned_member_value_bindings_from_expression(
                property,
                returned_identifier,
                local_aliases,
                bindings,
            );
            collect_returned_member_value_bindings_from_expression(
                value,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Expression::Call { callee, arguments }
        | Expression::SuperCall { callee, arguments }
        | Expression::New { callee, arguments } => {
            collect_returned_member_value_bindings_from_expression(
                callee,
                returned_identifier,
                local_aliases,
                bindings,
            );
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        collect_returned_member_value_bindings_from_expression(
                            expression,
                            returned_identifier,
                            local_aliases,
                            bindings,
                        );
                    }
                }
            }

            let Expression::Member { object, property } = callee.as_ref() else {
                return;
            };
            if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Object") {
                return;
            }
            if !matches!(property.as_ref(), Expression::String(name) if name == "defineProperty") {
                return;
            }
            let [
                CallArgument::Expression(target),
                CallArgument::Expression(property),
                CallArgument::Expression(descriptor),
                ..,
            ] = arguments.as_slice()
            else {
                return;
            };
            let Some(Expression::String(property_name)) =
                resolve_returned_member_value_property_key(
                    target,
                    property,
                    returned_identifier,
                    local_aliases,
                )
            else {
                return;
            };
            let Some(value) = resolve_returned_member_value_from_descriptor(descriptor) else {
                bindings.remove(&property_name);
                return;
            };
            bindings.insert(property_name, value);
        }
        Expression::Unary { expression, .. }
        | Expression::Await(expression)
        | Expression::EnumerateKeys(expression)
        | Expression::GetIterator(expression)
        | Expression::IteratorClose(expression) => {
            collect_returned_member_value_bindings_from_expression(
                expression,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Expression::Binary { left, right, .. } => {
            collect_returned_member_value_bindings_from_expression(
                left,
                returned_identifier,
                local_aliases,
                bindings,
            );
            collect_returned_member_value_bindings_from_expression(
                right,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            collect_returned_member_value_bindings_from_expression(
                condition,
                returned_identifier,
                local_aliases,
                bindings,
            );
            collect_returned_member_value_bindings_from_expression(
                then_expression,
                returned_identifier,
                local_aliases,
                bindings,
            );
            collect_returned_member_value_bindings_from_expression(
                else_expression,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Expression::Sequence(expressions) => {
            for expression in expressions {
                collect_returned_member_value_bindings_from_expression(
                    expression,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
        }
        Expression::Member { object, property } => {
            collect_returned_member_value_bindings_from_expression(
                object,
                returned_identifier,
                local_aliases,
                bindings,
            );
            collect_returned_member_value_bindings_from_expression(
                property,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Expression::Assign { value, .. } | Expression::AssignSuperMember { value, .. } => {
            collect_returned_member_value_bindings_from_expression(
                value,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Expression::SuperMember { property } => {
            collect_returned_member_value_bindings_from_expression(
                property,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Expression::Array(elements) => {
            for element in elements {
                match element {
                    crate::ir::hir::ArrayElement::Expression(expression)
                    | crate::ir::hir::ArrayElement::Spread(expression) => {
                        collect_returned_member_value_bindings_from_expression(
                            expression,
                            returned_identifier,
                            local_aliases,
                            bindings,
                        );
                    }
                }
            }
        }
        Expression::Object(entries) => {
            for entry in entries {
                match entry {
                    crate::ir::hir::ObjectEntry::Data { key, value } => {
                        collect_returned_member_value_bindings_from_expression(
                            key,
                            returned_identifier,
                            local_aliases,
                            bindings,
                        );
                        collect_returned_member_value_bindings_from_expression(
                            value,
                            returned_identifier,
                            local_aliases,
                            bindings,
                        );
                    }
                    crate::ir::hir::ObjectEntry::Getter { key, getter }
                    | crate::ir::hir::ObjectEntry::Setter {
                        key,
                        setter: getter,
                    } => {
                        collect_returned_member_value_bindings_from_expression(
                            key,
                            returned_identifier,
                            local_aliases,
                            bindings,
                        );
                        collect_returned_member_value_bindings_from_expression(
                            getter,
                            returned_identifier,
                            local_aliases,
                            bindings,
                        );
                    }
                    crate::ir::hir::ObjectEntry::Spread(value) => {
                        collect_returned_member_value_bindings_from_expression(
                            value,
                            returned_identifier,
                            local_aliases,
                            bindings,
                        );
                    }
                }
            }
        }
        Expression::Number(_)
        | Expression::BigInt(_)
        | Expression::String(_)
        | Expression::Bool(_)
        | Expression::Null
        | Expression::Undefined
        | Expression::NewTarget
        | Expression::Identifier(_)
        | Expression::This
        | Expression::Sent
        | Expression::Update { .. } => {}
    }
}

fn resolve_returned_member_value_property_key(
    object: &Expression,
    property: &Expression,
    returned_identifier: &str,
    local_aliases: &HashMap<String, Expression>,
) -> Option<Expression> {
    let resolved_property = resolve_returned_member_local_alias_expression(property, local_aliases);
    let property_key = match resolved_property {
        Expression::String(property_name) => Expression::String(property_name),
        _ => return None,
    };

    match object {
        Expression::Identifier(name) if name == returned_identifier => Some(property_key),
        _ => None,
    }
}

fn resolve_returned_member_value_from_descriptor(descriptor: &Expression) -> Option<Expression> {
    resolve_property_descriptor_definition(descriptor)?.value
}

fn collect_returned_member_function_bindings_from_statement(
    statement: &Statement,
    returned_identifier: &str,
    function_names: &HashSet<String>,
    bindings: &mut HashMap<ReturnedMemberFunctionBindingKey, LocalFunctionBinding>,
) {
    match statement {
        Statement::Block { body }
        | Statement::Labeled { body, .. }
        | Statement::With { body, .. } => {
            for statement in body {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
        }
        Statement::Var { value, .. }
        | Statement::Let { value, .. }
        | Statement::Assign { value, .. }
        | Statement::Expression(value)
        | Statement::Throw(value)
        | Statement::Return(value)
        | Statement::Yield { value }
        | Statement::YieldDelegate { value } => {
            collect_returned_member_function_bindings_from_expression(
                value,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Statement::AssignMember {
            object,
            property,
            value,
        } => {
            collect_returned_member_function_bindings_from_expression(
                object,
                returned_identifier,
                function_names,
                bindings,
            );
            collect_returned_member_function_bindings_from_expression(
                property,
                returned_identifier,
                function_names,
                bindings,
            );
            collect_returned_member_function_bindings_from_expression(
                value,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Statement::Print { values } => {
            for value in values {
                collect_returned_member_function_bindings_from_expression(
                    value,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_returned_member_function_bindings_from_expression(
                condition,
                returned_identifier,
                function_names,
                bindings,
            );
            for statement in then_branch {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            for statement in else_branch {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
        }
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => {
            for statement in body {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            for statement in catch_setup {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            for statement in catch_body {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
        }
        Statement::Switch {
            discriminant,
            cases,
            ..
        } => {
            collect_returned_member_function_bindings_from_expression(
                discriminant,
                returned_identifier,
                function_names,
                bindings,
            );
            for case in cases {
                if let Some(test) = &case.test {
                    collect_returned_member_function_bindings_from_expression(
                        test,
                        returned_identifier,
                        function_names,
                        bindings,
                    );
                }
                for statement in &case.body {
                    collect_returned_member_function_bindings_from_statement(
                        statement,
                        returned_identifier,
                        function_names,
                        bindings,
                    );
                }
            }
        }
        Statement::For {
            init,
            condition,
            update,
            break_hook,
            body,
            ..
        } => {
            for statement in init {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            if let Some(condition) = condition {
                collect_returned_member_function_bindings_from_expression(
                    condition,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            if let Some(update) = update {
                collect_returned_member_function_bindings_from_expression(
                    update,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            if let Some(break_hook) = break_hook {
                collect_returned_member_function_bindings_from_expression(
                    break_hook,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            for statement in body {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
        }
        Statement::While {
            condition,
            break_hook,
            body,
            ..
        }
        | Statement::DoWhile {
            condition,
            break_hook,
            body,
            ..
        } => {
            collect_returned_member_function_bindings_from_expression(
                condition,
                returned_identifier,
                function_names,
                bindings,
            );
            if let Some(break_hook) = break_hook {
                collect_returned_member_function_bindings_from_expression(
                    break_hook,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            for statement in body {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
        }
        _ => {}
    }
}

fn collect_returned_member_function_bindings_from_expression(
    expression: &Expression,
    returned_identifier: &str,
    function_names: &HashSet<String>,
    bindings: &mut HashMap<ReturnedMemberFunctionBindingKey, LocalFunctionBinding>,
) {
    match expression {
        Expression::Call { callee, arguments }
        | Expression::SuperCall { callee, arguments }
        | Expression::New { callee, arguments } => {
            collect_returned_member_function_bindings_from_expression(
                callee,
                returned_identifier,
                function_names,
                bindings,
            );
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        collect_returned_member_function_bindings_from_expression(
                            expression,
                            returned_identifier,
                            function_names,
                            bindings,
                        );
                    }
                }
            }

            let Expression::Member { object, property } = callee.as_ref() else {
                return;
            };
            if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Object") {
                return;
            }
            if !matches!(property.as_ref(), Expression::String(name) if name == "defineProperty") {
                return;
            }
            let [
                CallArgument::Expression(target),
                CallArgument::Expression(property),
                CallArgument::Expression(descriptor),
                ..,
            ] = arguments.as_slice()
            else {
                return;
            };
            let Some(key) =
                returned_member_function_binding_key(target, property, returned_identifier)
            else {
                return;
            };
            let Some(binding) = resolve_returned_member_function_binding_from_descriptor(
                descriptor,
                returned_identifier,
                function_names,
                bindings,
            ) else {
                bindings.remove(&key);
                return;
            };
            bindings.insert(key, binding);
        }
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            collect_returned_member_function_bindings_from_expression(
                object,
                returned_identifier,
                function_names,
                bindings,
            );
            collect_returned_member_function_bindings_from_expression(
                property,
                returned_identifier,
                function_names,
                bindings,
            );
            collect_returned_member_function_bindings_from_expression(
                value,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Expression::Unary { expression, .. }
        | Expression::Await(expression)
        | Expression::EnumerateKeys(expression)
        | Expression::GetIterator(expression)
        | Expression::IteratorClose(expression) => {
            collect_returned_member_function_bindings_from_expression(
                expression,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Expression::Binary { left, right, .. } => {
            collect_returned_member_function_bindings_from_expression(
                left,
                returned_identifier,
                function_names,
                bindings,
            );
            collect_returned_member_function_bindings_from_expression(
                right,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            collect_returned_member_function_bindings_from_expression(
                condition,
                returned_identifier,
                function_names,
                bindings,
            );
            collect_returned_member_function_bindings_from_expression(
                then_expression,
                returned_identifier,
                function_names,
                bindings,
            );
            collect_returned_member_function_bindings_from_expression(
                else_expression,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Expression::Sequence(expressions) => {
            for expression in expressions {
                collect_returned_member_function_bindings_from_expression(
                    expression,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
        }
        Expression::Member { object, property } => {
            collect_returned_member_function_bindings_from_expression(
                object,
                returned_identifier,
                function_names,
                bindings,
            );
            collect_returned_member_function_bindings_from_expression(
                property,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Expression::Assign { value, .. } | Expression::AssignSuperMember { value, .. } => {
            collect_returned_member_function_bindings_from_expression(
                value,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Expression::SuperMember { property } => {
            collect_returned_member_function_bindings_from_expression(
                property,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Expression::Array(elements) => {
            for element in elements {
                match element {
                    crate::ir::hir::ArrayElement::Expression(expression)
                    | crate::ir::hir::ArrayElement::Spread(expression) => {
                        collect_returned_member_function_bindings_from_expression(
                            expression,
                            returned_identifier,
                            function_names,
                            bindings,
                        );
                    }
                }
            }
        }
        Expression::Object(entries) => {
            for entry in entries {
                match entry {
                    crate::ir::hir::ObjectEntry::Data { key, value } => {
                        collect_returned_member_function_bindings_from_expression(
                            key,
                            returned_identifier,
                            function_names,
                            bindings,
                        );
                        collect_returned_member_function_bindings_from_expression(
                            value,
                            returned_identifier,
                            function_names,
                            bindings,
                        );
                    }
                    crate::ir::hir::ObjectEntry::Getter { key, getter }
                    | crate::ir::hir::ObjectEntry::Setter {
                        key,
                        setter: getter,
                    } => {
                        collect_returned_member_function_bindings_from_expression(
                            key,
                            returned_identifier,
                            function_names,
                            bindings,
                        );
                        collect_returned_member_function_bindings_from_expression(
                            getter,
                            returned_identifier,
                            function_names,
                            bindings,
                        );
                    }
                    crate::ir::hir::ObjectEntry::Spread(value) => {
                        collect_returned_member_function_bindings_from_expression(
                            value,
                            returned_identifier,
                            function_names,
                            bindings,
                        );
                    }
                }
            }
        }
        Expression::Number(_)
        | Expression::BigInt(_)
        | Expression::String(_)
        | Expression::Bool(_)
        | Expression::Null
        | Expression::Undefined
        | Expression::NewTarget
        | Expression::Identifier(_)
        | Expression::This
        | Expression::Sent
        | Expression::Update { .. } => {}
    }
}

fn returned_member_function_binding_key(
    object: &Expression,
    property: &Expression,
    returned_identifier: &str,
) -> Option<ReturnedMemberFunctionBindingKey> {
    let Expression::String(property_name) = property else {
        return None;
    };

    let target = match object {
        Expression::Identifier(name) if name == returned_identifier => {
            ReturnedMemberFunctionBindingTarget::Value
        }
        Expression::Member { object, property }
            if matches!(property.as_ref(), Expression::String(name) if name == "prototype")
                && matches!(object.as_ref(), Expression::Identifier(name) if name == returned_identifier) =>
        {
            ReturnedMemberFunctionBindingTarget::Prototype
        }
        _ => return None,
    };

    Some(ReturnedMemberFunctionBindingKey {
        target,
        property: property_name.clone(),
    })
}

fn resolve_returned_member_function_binding_from_descriptor(
    descriptor: &Expression,
    returned_identifier: &str,
    function_names: &HashSet<String>,
    bindings: &HashMap<ReturnedMemberFunctionBindingKey, LocalFunctionBinding>,
) -> Option<LocalFunctionBinding> {
    let Expression::Object(entries) = descriptor else {
        return None;
    };
    for entry in entries {
        let crate::ir::hir::ObjectEntry::Data { key, value } = entry else {
            continue;
        };
        if matches!(key, Expression::String(name) if name == "value") {
            return resolve_returned_member_function_binding(
                value,
                returned_identifier,
                function_names,
                bindings,
            );
        }
    }
    None
}

fn collect_inline_function_summary(
    function: &FunctionDeclaration,
) -> Option<InlineFunctionSummary> {
    let mut summary = InlineFunctionSummary::default();
    let parameter_names = function
        .params
        .iter()
        .map(|param| param.name.clone())
        .collect::<HashSet<_>>();
    let mut local_bindings = HashMap::new();
    for statement in &function.body {
        match statement {
            Statement::Var { name, value } | Statement::Let { name, value, .. } => {
                if parameter_names.contains(name) {
                    return None;
                }
                local_bindings.insert(
                    name.clone(),
                    substitute_inline_summary_bindings(value, &local_bindings),
                );
            }
            Statement::Assign { name, value } => {
                if parameter_names.contains(name) {
                    return None;
                }
                if local_bindings.contains_key(name) {
                    return None;
                }
                summary.effects.push(InlineFunctionEffect::Assign {
                    name: name.clone(),
                    value: substitute_inline_summary_bindings(value, &local_bindings),
                });
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                let object = substitute_inline_summary_bindings(object, &local_bindings);
                let property = substitute_inline_summary_bindings(property, &local_bindings);
                let value = substitute_inline_summary_bindings(value, &local_bindings);
                if !function.mapped_arguments
                    && matches!(&object, Expression::Identifier(name) if name == "arguments")
                    && inline_summary_side_effect_free_expression(&property)
                    && inline_summary_side_effect_free_expression(&value)
                {
                    continue;
                }
                summary
                    .effects
                    .push(InlineFunctionEffect::Expression(Expression::AssignMember {
                        object: Box::new(object),
                        property: Box::new(property),
                        value: Box::new(value),
                    }));
            }
            Statement::Expression(Expression::Update { name, op, prefix }) => {
                if function.params.iter().any(|param| param.name == *name)
                    || local_bindings.contains_key(name)
                {
                    return None;
                }
                summary.effects.push(InlineFunctionEffect::Update {
                    name: name.clone(),
                    op: *op,
                    prefix: *prefix,
                });
            }
            Statement::Expression(expression) => {
                summary.effects.push(InlineFunctionEffect::Expression(
                    substitute_inline_summary_bindings(expression, &local_bindings),
                ))
            }
            Statement::Return(value) => {
                if summary.return_value.is_some() {
                    return None;
                }
                summary.return_value =
                    Some(substitute_inline_summary_bindings(value, &local_bindings));
            }
            Statement::Block { body } if body.is_empty() => {}
            _ => return None,
        }
    }

    Some(summary)
}

fn rewrite_inline_function_summary_bindings(
    summary: &InlineFunctionSummary,
    bindings: &HashMap<String, Expression>,
) -> InlineFunctionSummary {
    InlineFunctionSummary {
        effects: summary
            .effects
            .iter()
            .map(|effect| match effect {
                InlineFunctionEffect::Assign { name, value } => InlineFunctionEffect::Assign {
                    name: name.clone(),
                    value: substitute_inline_summary_bindings(value, bindings),
                },
                InlineFunctionEffect::Update { name, op, prefix } => InlineFunctionEffect::Update {
                    name: name.clone(),
                    op: *op,
                    prefix: *prefix,
                },
                InlineFunctionEffect::Expression(expression) => InlineFunctionEffect::Expression(
                    substitute_inline_summary_bindings(expression, bindings),
                ),
            })
            .collect(),
        return_value: summary
            .return_value
            .as_ref()
            .map(|value| substitute_inline_summary_bindings(value, bindings)),
    }
}

fn substitute_inline_summary_bindings(
    expression: &Expression,
    bindings: &HashMap<String, Expression>,
) -> Expression {
    match expression {
        Expression::Identifier(name) => bindings
            .get(name)
            .cloned()
            .unwrap_or_else(|| expression.clone()),
        Expression::Member { object, property } => Expression::Member {
            object: Box::new(substitute_inline_summary_bindings(object, bindings)),
            property: Box::new(substitute_inline_summary_bindings(property, bindings)),
        },
        Expression::SuperMember { property } => Expression::SuperMember {
            property: Box::new(substitute_inline_summary_bindings(property, bindings)),
        },
        Expression::Assign { name, value } => Expression::Assign {
            name: name.clone(),
            value: Box::new(substitute_inline_summary_bindings(value, bindings)),
        },
        Expression::AssignMember {
            object,
            property,
            value,
        } => Expression::AssignMember {
            object: Box::new(substitute_inline_summary_bindings(object, bindings)),
            property: Box::new(substitute_inline_summary_bindings(property, bindings)),
            value: Box::new(substitute_inline_summary_bindings(value, bindings)),
        },
        Expression::AssignSuperMember { property, value } => Expression::AssignSuperMember {
            property: Box::new(substitute_inline_summary_bindings(property, bindings)),
            value: Box::new(substitute_inline_summary_bindings(value, bindings)),
        },
        Expression::Await(value) => Expression::Await(Box::new(
            substitute_inline_summary_bindings(value, bindings),
        )),
        Expression::EnumerateKeys(value) => Expression::EnumerateKeys(Box::new(
            substitute_inline_summary_bindings(value, bindings),
        )),
        Expression::GetIterator(value) => Expression::GetIterator(Box::new(
            substitute_inline_summary_bindings(value, bindings),
        )),
        Expression::IteratorClose(value) => Expression::IteratorClose(Box::new(
            substitute_inline_summary_bindings(value, bindings),
        )),
        Expression::Unary { op, expression } => Expression::Unary {
            op: *op,
            expression: Box::new(substitute_inline_summary_bindings(expression, bindings)),
        },
        Expression::Binary { op, left, right } => Expression::Binary {
            op: *op,
            left: Box::new(substitute_inline_summary_bindings(left, bindings)),
            right: Box::new(substitute_inline_summary_bindings(right, bindings)),
        },
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => Expression::Conditional {
            condition: Box::new(substitute_inline_summary_bindings(condition, bindings)),
            then_expression: Box::new(substitute_inline_summary_bindings(
                then_expression,
                bindings,
            )),
            else_expression: Box::new(substitute_inline_summary_bindings(
                else_expression,
                bindings,
            )),
        },
        Expression::Sequence(expressions) => Expression::Sequence(
            expressions
                .iter()
                .map(|expression| substitute_inline_summary_bindings(expression, bindings))
                .collect(),
        ),
        Expression::Call { callee, arguments } => Expression::Call {
            callee: Box::new(substitute_inline_summary_bindings(callee, bindings)),
            arguments: arguments
                .iter()
                .map(|argument| match argument {
                    CallArgument::Expression(expression) => CallArgument::Expression(
                        substitute_inline_summary_bindings(expression, bindings),
                    ),
                    CallArgument::Spread(expression) => CallArgument::Spread(
                        substitute_inline_summary_bindings(expression, bindings),
                    ),
                })
                .collect(),
        },
        Expression::SuperCall { callee, arguments } => Expression::SuperCall {
            callee: Box::new(substitute_inline_summary_bindings(callee, bindings)),
            arguments: arguments
                .iter()
                .map(|argument| match argument {
                    CallArgument::Expression(expression) => CallArgument::Expression(
                        substitute_inline_summary_bindings(expression, bindings),
                    ),
                    CallArgument::Spread(expression) => CallArgument::Spread(
                        substitute_inline_summary_bindings(expression, bindings),
                    ),
                })
                .collect(),
        },
        Expression::New { callee, arguments } => Expression::New {
            callee: Box::new(substitute_inline_summary_bindings(callee, bindings)),
            arguments: arguments
                .iter()
                .map(|argument| match argument {
                    CallArgument::Expression(expression) => CallArgument::Expression(
                        substitute_inline_summary_bindings(expression, bindings),
                    ),
                    CallArgument::Spread(expression) => CallArgument::Spread(
                        substitute_inline_summary_bindings(expression, bindings),
                    ),
                })
                .collect(),
        },
        Expression::Array(elements) => Expression::Array(
            elements
                .iter()
                .map(|element| match element {
                    crate::ir::hir::ArrayElement::Expression(expression) => {
                        crate::ir::hir::ArrayElement::Expression(substitute_inline_summary_bindings(
                            expression, bindings,
                        ))
                    }
                    crate::ir::hir::ArrayElement::Spread(expression) => {
                        crate::ir::hir::ArrayElement::Spread(substitute_inline_summary_bindings(
                            expression, bindings,
                        ))
                    }
                })
                .collect(),
        ),
        Expression::Object(entries) => Expression::Object(
            entries
                .iter()
                .map(|entry| match entry {
                    crate::ir::hir::ObjectEntry::Data { key, value } => crate::ir::hir::ObjectEntry::Data {
                        key: substitute_inline_summary_bindings(key, bindings),
                        value: substitute_inline_summary_bindings(value, bindings),
                    },
                    crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                        crate::ir::hir::ObjectEntry::Getter {
                            key: substitute_inline_summary_bindings(key, bindings),
                            getter: substitute_inline_summary_bindings(getter, bindings),
                        }
                    }
                    crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                        crate::ir::hir::ObjectEntry::Setter {
                            key: substitute_inline_summary_bindings(key, bindings),
                            setter: substitute_inline_summary_bindings(setter, bindings),
                        }
                    }
                    crate::ir::hir::ObjectEntry::Spread(expression) => crate::ir::hir::ObjectEntry::Spread(
                        substitute_inline_summary_bindings(expression, bindings),
                    ),
                })
                .collect(),
        ),
        _ => expression.clone(),
    }
}

fn inline_summary_side_effect_free_expression(expression: &Expression) -> bool {
    match expression {
        Expression::Number(_)
        | Expression::BigInt(_)
        | Expression::String(_)
        | Expression::Bool(_)
        | Expression::Null
        | Expression::Undefined
        | Expression::Identifier(_)
        | Expression::This
        | Expression::NewTarget
        | Expression::Sent => true,
        Expression::Member { object, property } => {
            inline_summary_side_effect_free_expression(object)
                && inline_summary_side_effect_free_expression(property)
        }
        Expression::SuperMember { property } => {
            inline_summary_side_effect_free_expression(property)
        }
        Expression::Unary { expression, .. }
        | Expression::Await(expression)
        | Expression::EnumerateKeys(expression)
        | Expression::GetIterator(expression)
        | Expression::IteratorClose(expression) => {
            inline_summary_side_effect_free_expression(expression)
        }
        Expression::Binary { left, right, .. } => {
            inline_summary_side_effect_free_expression(left)
                && inline_summary_side_effect_free_expression(right)
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            inline_summary_side_effect_free_expression(condition)
                && inline_summary_side_effect_free_expression(then_expression)
                && inline_summary_side_effect_free_expression(else_expression)
        }
        Expression::Sequence(expressions) => expressions
            .iter()
            .all(inline_summary_side_effect_free_expression),
        Expression::Array(elements) => elements.iter().all(|element| match element {
            crate::ir::hir::ArrayElement::Expression(expression)
            | crate::ir::hir::ArrayElement::Spread(expression) => {
                inline_summary_side_effect_free_expression(expression)
            }
        }),
        Expression::Object(entries) => entries.iter().all(|entry| match entry {
            crate::ir::hir::ObjectEntry::Data { key, value } => {
                inline_summary_side_effect_free_expression(key)
                    && inline_summary_side_effect_free_expression(value)
            }
            crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                inline_summary_side_effect_free_expression(key)
                    && inline_summary_side_effect_free_expression(getter)
            }
            crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                inline_summary_side_effect_free_expression(key)
                    && inline_summary_side_effect_free_expression(setter)
            }
            crate::ir::hir::ObjectEntry::Spread(expression) => {
                inline_summary_side_effect_free_expression(expression)
            }
        }),
        Expression::Assign { .. }
        | Expression::AssignMember { .. }
        | Expression::AssignSuperMember { .. }
        | Expression::Call { .. }
        | Expression::SuperCall { .. }
        | Expression::New { .. }
        | Expression::Update { .. } => false,
    }
}

fn static_expression_matches(lhs: &Expression, rhs: &Expression) -> bool {
    match (lhs, rhs) {
        (Expression::Number(left), Expression::Number(right)) => {
            (left.is_nan() && right.is_nan()) || left == right
        }
        _ => lhs == rhs,
    }
}

fn expression_mentions_call_frame_state(expression: &Expression) -> bool {
    match expression {
        Expression::Identifier(name) => name == "arguments",
        Expression::Member { object, property } => {
            expression_mentions_call_frame_state(object)
                || expression_mentions_call_frame_state(property)
        }
        Expression::Assign { value, .. } => expression_mentions_call_frame_state(value),
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            expression_mentions_call_frame_state(object)
                || expression_mentions_call_frame_state(property)
                || expression_mentions_call_frame_state(value)
        }
        Expression::AssignSuperMember { property, value } => {
            expression_mentions_call_frame_state(property)
                || expression_mentions_call_frame_state(value)
        }
        Expression::Await(expression)
        | Expression::EnumerateKeys(expression)
        | Expression::GetIterator(expression)
        | Expression::IteratorClose(expression)
        | Expression::Unary { expression, .. } => expression_mentions_call_frame_state(expression),
        Expression::Binary { left, right, .. } => {
            expression_mentions_call_frame_state(left)
                || expression_mentions_call_frame_state(right)
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            expression_mentions_call_frame_state(condition)
                || expression_mentions_call_frame_state(then_expression)
                || expression_mentions_call_frame_state(else_expression)
        }
        Expression::Sequence(expressions) => {
            expressions.iter().any(expression_mentions_call_frame_state)
        }
        Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
            matches!(callee.as_ref(), Expression::Identifier(name) if name == "eval")
                || matches!(
                    callee.as_ref(),
                    Expression::Sequence(expressions)
                        if matches!(expressions.last(), Some(Expression::Identifier(name)) if name == "eval")
                )
                || expression_mentions_call_frame_state(callee)
                || arguments.iter().any(|argument| match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        expression_mentions_call_frame_state(expression)
                    }
                })
        }
        Expression::SuperCall { .. } => true,
        Expression::Array(elements) => elements.iter().any(|element| match element {
            crate::ir::hir::ArrayElement::Expression(expression)
            | crate::ir::hir::ArrayElement::Spread(expression) => {
                expression_mentions_call_frame_state(expression)
            }
        }),
        Expression::Object(entries) => entries.iter().any(|entry| match entry {
            crate::ir::hir::ObjectEntry::Data { key, value } => {
                expression_mentions_call_frame_state(key)
                    || expression_mentions_call_frame_state(value)
            }
            crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                expression_mentions_call_frame_state(key)
                    || expression_mentions_call_frame_state(getter)
            }
            crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                expression_mentions_call_frame_state(key)
                    || expression_mentions_call_frame_state(setter)
            }
            crate::ir::hir::ObjectEntry::Spread(expression) => {
                expression_mentions_call_frame_state(expression)
            }
        }),
        Expression::SuperMember { .. } => false,
        Expression::This | Expression::NewTarget | Expression::Sent => true,
        Expression::Update { .. }
        | Expression::Number(_)
        | Expression::BigInt(_)
        | Expression::String(_)
        | Expression::Bool(_)
        | Expression::Null
        | Expression::Undefined => false,
    }
}

fn inline_summary_mentions_call_frame_state(summary: &InlineFunctionSummary) -> bool {
    summary.effects.iter().any(|effect| match effect {
        InlineFunctionEffect::Assign { value, .. } => expression_mentions_call_frame_state(value),
        InlineFunctionEffect::Update { .. } => false,
        InlineFunctionEffect::Expression(expression) => {
            expression_mentions_call_frame_state(expression)
        }
    }) || summary
        .return_value
        .as_ref()
        .is_some_and(expression_mentions_call_frame_state)
}

fn expression_mentions_assertion_builtin(expression: &Expression) -> bool {
    match expression {
        Expression::Identifier(name) => matches!(
            name.as_str(),
            "__assert" | "__assertSameValue" | "__assertNotSameValue" | "__ayyAssertThrows"
        ),
        Expression::Member { object, property } => {
            (matches!(object.as_ref(), Expression::Identifier(name) if name == "assert")
                && matches!(
                    property.as_ref(),
                    Expression::String(name)
                        if matches!(name.as_str(), "sameValue" | "notSameValue")
                ))
                || expression_mentions_assertion_builtin(object)
                || expression_mentions_assertion_builtin(property)
        }
        Expression::SuperMember { property } => expression_mentions_assertion_builtin(property),
        Expression::Assign { value, .. }
        | Expression::Await(value)
        | Expression::EnumerateKeys(value)
        | Expression::GetIterator(value)
        | Expression::IteratorClose(value)
        | Expression::Unary {
            expression: value, ..
        } => expression_mentions_assertion_builtin(value),
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            expression_mentions_assertion_builtin(object)
                || expression_mentions_assertion_builtin(property)
                || expression_mentions_assertion_builtin(value)
        }
        Expression::AssignSuperMember { property, value } => {
            expression_mentions_assertion_builtin(property)
                || expression_mentions_assertion_builtin(value)
        }
        Expression::Binary { left, right, .. } => {
            expression_mentions_assertion_builtin(left)
                || expression_mentions_assertion_builtin(right)
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            expression_mentions_assertion_builtin(condition)
                || expression_mentions_assertion_builtin(then_expression)
                || expression_mentions_assertion_builtin(else_expression)
        }
        Expression::Sequence(expressions) => expressions
            .iter()
            .any(expression_mentions_assertion_builtin),
        Expression::Call { callee, arguments }
        | Expression::SuperCall { callee, arguments }
        | Expression::New { callee, arguments } => {
            expression_mentions_assertion_builtin(callee)
                || arguments.iter().any(|argument| match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        expression_mentions_assertion_builtin(expression)
                    }
                })
        }
        Expression::Array(elements) => elements.iter().any(|element| match element {
            ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                expression_mentions_assertion_builtin(expression)
            }
        }),
        Expression::Object(entries) => entries.iter().any(|entry| match entry {
            ObjectEntry::Data { key, value } => {
                expression_mentions_assertion_builtin(key)
                    || expression_mentions_assertion_builtin(value)
            }
            ObjectEntry::Getter { key, getter } => {
                expression_mentions_assertion_builtin(key)
                    || expression_mentions_assertion_builtin(getter)
            }
            ObjectEntry::Setter { key, setter } => {
                expression_mentions_assertion_builtin(key)
                    || expression_mentions_assertion_builtin(setter)
            }
            ObjectEntry::Spread(expression) => expression_mentions_assertion_builtin(expression),
        }),
        Expression::Update { .. }
        | Expression::Number(_)
        | Expression::BigInt(_)
        | Expression::String(_)
        | Expression::Bool(_)
        | Expression::Null
        | Expression::Undefined
        | Expression::NewTarget
        | Expression::This
        | Expression::Sent => false,
    }
}

fn inline_summary_mentions_assertion_builtin(summary: &InlineFunctionSummary) -> bool {
    summary.effects.iter().any(|effect| match effect {
        InlineFunctionEffect::Assign { value, .. } => expression_mentions_assertion_builtin(value),
        InlineFunctionEffect::Update { .. } => false,
        InlineFunctionEffect::Expression(expression) => {
            expression_mentions_assertion_builtin(expression)
        }
    }) || summary
        .return_value
        .as_ref()
        .is_some_and(expression_mentions_assertion_builtin)
}

fn resolve_returned_member_function_binding(
    expression: &Expression,
    returned_identifier: &str,
    function_names: &HashSet<String>,
    bindings: &HashMap<ReturnedMemberFunctionBindingKey, LocalFunctionBinding>,
) -> Option<LocalFunctionBinding> {
    match expression {
        Expression::Identifier(name) if function_names.contains(name) => {
            Some(LocalFunctionBinding::User(name.clone()))
        }
        Expression::Member { object, property } => {
            let key = returned_member_function_binding_key(object, property, returned_identifier)?;
            bindings.get(&key).cloned()
        }
        _ => None,
    }
}

fn collect_arguments_usage_from_statement(
    statement: &Statement,
    indexed_slots: &mut HashSet<u32>,
    track_all_slots: &mut bool,
) {
    match statement {
        Statement::Block { body } | Statement::Labeled { body, .. } => {
            for statement in body {
                collect_arguments_usage_from_statement(statement, indexed_slots, track_all_slots);
            }
        }
        Statement::Var { value, .. }
        | Statement::Let { value, .. }
        | Statement::Assign { value, .. }
        | Statement::Expression(value)
        | Statement::Throw(value)
        | Statement::Return(value)
        | Statement::Yield { value }
        | Statement::YieldDelegate { value } => {
            collect_arguments_usage_from_expression(value, indexed_slots, track_all_slots);
        }
        Statement::AssignMember {
            object,
            property,
            value,
        } => {
            collect_arguments_usage_from_expression(object, indexed_slots, track_all_slots);
            collect_arguments_usage_from_expression(property, indexed_slots, track_all_slots);
            collect_arguments_usage_from_expression(value, indexed_slots, track_all_slots);
        }
        Statement::Print { values } => {
            for value in values {
                collect_arguments_usage_from_expression(value, indexed_slots, track_all_slots);
            }
        }
        Statement::With { object, body } => {
            collect_arguments_usage_from_expression(object, indexed_slots, track_all_slots);
            for statement in body {
                collect_arguments_usage_from_statement(statement, indexed_slots, track_all_slots);
            }
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_arguments_usage_from_expression(condition, indexed_slots, track_all_slots);
            for statement in then_branch {
                collect_arguments_usage_from_statement(statement, indexed_slots, track_all_slots);
            }
            for statement in else_branch {
                collect_arguments_usage_from_statement(statement, indexed_slots, track_all_slots);
            }
        }
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => {
            for statement in body {
                collect_arguments_usage_from_statement(statement, indexed_slots, track_all_slots);
            }
            for statement in catch_setup {
                collect_arguments_usage_from_statement(statement, indexed_slots, track_all_slots);
            }
            for statement in catch_body {
                collect_arguments_usage_from_statement(statement, indexed_slots, track_all_slots);
            }
        }
        Statement::Switch {
            discriminant,
            cases,
            ..
        } => {
            collect_arguments_usage_from_expression(discriminant, indexed_slots, track_all_slots);
            for case in cases {
                if let Some(test) = &case.test {
                    collect_arguments_usage_from_expression(test, indexed_slots, track_all_slots);
                }
                for statement in &case.body {
                    collect_arguments_usage_from_statement(
                        statement,
                        indexed_slots,
                        track_all_slots,
                    );
                }
            }
        }
        Statement::For {
            init,
            condition,
            update,
            break_hook,
            body,
            ..
        } => {
            for statement in init {
                collect_arguments_usage_from_statement(statement, indexed_slots, track_all_slots);
            }
            if let Some(condition) = condition {
                collect_arguments_usage_from_expression(condition, indexed_slots, track_all_slots);
            }
            if let Some(update) = update {
                collect_arguments_usage_from_expression(update, indexed_slots, track_all_slots);
            }
            if let Some(break_hook) = break_hook {
                collect_arguments_usage_from_expression(break_hook, indexed_slots, track_all_slots);
            }
            for statement in body {
                collect_arguments_usage_from_statement(statement, indexed_slots, track_all_slots);
            }
        }
        Statement::While {
            condition,
            break_hook,
            body,
            ..
        }
        | Statement::DoWhile {
            condition,
            break_hook,
            body,
            ..
        } => {
            collect_arguments_usage_from_expression(condition, indexed_slots, track_all_slots);
            if let Some(break_hook) = break_hook {
                collect_arguments_usage_from_expression(break_hook, indexed_slots, track_all_slots);
            }
            for statement in body {
                collect_arguments_usage_from_statement(statement, indexed_slots, track_all_slots);
            }
        }
        Statement::Break { .. } | Statement::Continue { .. } => {}
    }
}

fn collect_arguments_usage_from_expression(
    expression: &Expression,
    indexed_slots: &mut HashSet<u32>,
    track_all_slots: &mut bool,
) {
    match expression {
        Expression::Member { object, property } => {
            if is_arguments_identifier(object) {
                if let Some(index) = argument_index_from_expression(property) {
                    indexed_slots.insert(index);
                } else {
                    *track_all_slots = true;
                }
            }
            collect_arguments_usage_from_expression(object, indexed_slots, track_all_slots);
            collect_arguments_usage_from_expression(property, indexed_slots, track_all_slots);
        }
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            if is_arguments_identifier(object) {
                if let Some(index) = argument_index_from_expression(property) {
                    indexed_slots.insert(index);
                } else {
                    *track_all_slots = true;
                }
            }
            collect_arguments_usage_from_expression(object, indexed_slots, track_all_slots);
            collect_arguments_usage_from_expression(property, indexed_slots, track_all_slots);
            collect_arguments_usage_from_expression(value, indexed_slots, track_all_slots);
        }
        Expression::Assign { value, .. }
        | Expression::Await(value)
        | Expression::EnumerateKeys(value)
        | Expression::IteratorClose(value) => {
            collect_arguments_usage_from_expression(value, indexed_slots, track_all_slots);
        }
        Expression::GetIterator(value) => {
            if is_arguments_identifier(value) {
                *track_all_slots = true;
            }
            collect_arguments_usage_from_expression(value, indexed_slots, track_all_slots);
        }
        Expression::Unary { op, expression } => {
            if *op == UnaryOp::Delete {
                if let Expression::Member { object, property } = expression.as_ref() {
                    if is_arguments_identifier(object) {
                        if let Some(index) = argument_index_from_expression(property) {
                            indexed_slots.insert(index);
                        } else {
                            *track_all_slots = true;
                        }
                    }
                }
            }
            collect_arguments_usage_from_expression(expression, indexed_slots, track_all_slots);
        }
        Expression::Binary { left, right, .. } => {
            collect_arguments_usage_from_expression(left, indexed_slots, track_all_slots);
            collect_arguments_usage_from_expression(right, indexed_slots, track_all_slots);
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            collect_arguments_usage_from_expression(condition, indexed_slots, track_all_slots);
            collect_arguments_usage_from_expression(
                then_expression,
                indexed_slots,
                track_all_slots,
            );
            collect_arguments_usage_from_expression(
                else_expression,
                indexed_slots,
                track_all_slots,
            );
        }
        Expression::Sequence(expressions) => {
            for expression in expressions {
                collect_arguments_usage_from_expression(expression, indexed_slots, track_all_slots);
            }
        }
        Expression::Array(elements) => {
            for element in elements {
                match element {
                    crate::ir::hir::ArrayElement::Expression(expression)
                    | crate::ir::hir::ArrayElement::Spread(expression) => {
                        collect_arguments_usage_from_expression(
                            expression,
                            indexed_slots,
                            track_all_slots,
                        );
                    }
                }
            }
        }
        Expression::Object(entries) => {
            for entry in entries {
                match entry {
                    crate::ir::hir::ObjectEntry::Data { key, value } => {
                        collect_arguments_usage_from_expression(
                            key,
                            indexed_slots,
                            track_all_slots,
                        );
                        collect_arguments_usage_from_expression(
                            value,
                            indexed_slots,
                            track_all_slots,
                        );
                    }
                    crate::ir::hir::ObjectEntry::Getter { key, getter }
                    | crate::ir::hir::ObjectEntry::Setter {
                        key,
                        setter: getter,
                    } => {
                        collect_arguments_usage_from_expression(
                            key,
                            indexed_slots,
                            track_all_slots,
                        );
                        collect_arguments_usage_from_expression(
                            getter,
                            indexed_slots,
                            track_all_slots,
                        );
                    }
                    crate::ir::hir::ObjectEntry::Spread(value) => {
                        collect_arguments_usage_from_expression(
                            value,
                            indexed_slots,
                            track_all_slots,
                        );
                    }
                }
            }
        }
        Expression::Call { callee, arguments }
        | Expression::SuperCall { callee, arguments }
        | Expression::New { callee, arguments } => {
            collect_arguments_usage_from_expression(callee, indexed_slots, track_all_slots);
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        collect_arguments_usage_from_expression(
                            expression,
                            indexed_slots,
                            track_all_slots,
                        );
                    }
                }
            }
        }
        Expression::SuperMember { property } => {
            collect_arguments_usage_from_expression(property, indexed_slots, track_all_slots);
        }
        Expression::AssignSuperMember { property, value } => {
            collect_arguments_usage_from_expression(property, indexed_slots, track_all_slots);
            collect_arguments_usage_from_expression(value, indexed_slots, track_all_slots);
        }
        Expression::Number(_)
        | Expression::BigInt(_)
        | Expression::String(_)
        | Expression::Bool(_)
        | Expression::Null
        | Expression::Undefined
        | Expression::NewTarget
        | Expression::Identifier(_)
        | Expression::This
        | Expression::Sent
        | Expression::Update { .. } => {}
    }
}

