use super::*;

pub(in crate::backend::direct_wasm) fn infer_call_result_kind(
    name: &str,
) -> Option<StaticValueKind> {
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

pub(in crate::backend::direct_wasm) fn is_builtin_like_capture_identifier(name: &str) -> bool {
    name == "eval"
        || name == "console"
        || matches!(
            name,
            "__assert" | "__assertSameValue" | "__assertNotSameValue" | "__ayyAssertThrows"
        )
        || builtin_identifier_kind(name).is_some()
        || infer_call_result_kind(name).is_some()
}

pub(in crate::backend::direct_wasm) fn function_constructor_literal_source_parts(
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

pub(in crate::backend::direct_wasm) fn function_constructor_wrapper_sources(
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

pub(in crate::backend::direct_wasm) fn is_function_constructor_builtin(name: &str) -> bool {
    matches!(
        name,
        "Function"
            | "AsyncFunction"
            | "GeneratorFunction"
            | "AsyncGeneratorFunction"
            | FUNCTION_CONSTRUCTOR_FAMILY_BUILTIN
    )
}

pub(in crate::backend::direct_wasm) fn are_function_constructor_bindings(
    bindings: &[LocalFunctionBinding],
) -> bool {
    bindings.iter().all(|binding| {
        matches!(
            binding,
            LocalFunctionBinding::Builtin(name) if is_function_constructor_builtin(name)
        )
    })
}

pub(in crate::backend::direct_wasm) fn typed_array_builtin_bytes_per_element(
    name: &str,
) -> Option<u32> {
    match name {
        "Uint8Array" | "Int8Array" | "Uint8ClampedArray" => Some(1),
        "Uint16Array" | "Int16Array" => Some(2),
        "Uint32Array" | "Int32Array" | "Float32Array" => Some(4),
        "Float64Array" => Some(8),
        _ => None,
    }
}

pub(in crate::backend::direct_wasm) fn is_bytes_per_element_access(
    expression: &Expression,
) -> bool {
    matches!(
        expression,
        Expression::Member { property, .. }
            if matches!(property.as_ref(), Expression::String(name) if name == "BYTES_PER_ELEMENT")
    )
}

pub(in crate::backend::direct_wasm) fn extract_typed_array_element_count(
    expression: &Expression,
) -> Option<usize> {
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

pub(in crate::backend::direct_wasm) fn native_error_runtime_value(name: &str) -> Option<i32> {
    NATIVE_ERROR_NAMES
        .iter()
        .position(|candidate| *candidate == name)
        .map(|offset| JS_NATIVE_ERROR_VALUE_BASE + offset as i32)
}

pub(in crate::backend::direct_wasm) fn native_error_instanceof_values(
    name: &str,
) -> Option<Vec<i32>> {
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

pub(in crate::backend::direct_wasm) fn is_arguments_identifier(expression: &Expression) -> bool {
    matches!(expression, Expression::Identifier(name) if name == "arguments")
}

pub(in crate::backend::direct_wasm) fn is_symbol_iterator_expression(
    expression: &Expression,
) -> bool {
    matches!(
        expression,
        Expression::Member { object, property }
            if matches!(object.as_ref(), Expression::Identifier(name) if name == "Symbol")
                && matches!(property.as_ref(), Expression::String(name) if name == "iterator")
    )
}

pub(in crate::backend::direct_wasm) fn symbol_iterator_expression() -> Expression {
    Expression::Member {
        object: Box::new(Expression::Identifier("Symbol".to_string())),
        property: Box::new(Expression::String("iterator".to_string())),
    }
}

pub(in crate::backend::direct_wasm) fn arguments_symbol_iterator_expression() -> Expression {
    Expression::Member {
        object: Box::new(Expression::Array(Vec::new())),
        property: Box::new(symbol_iterator_expression()),
    }
}

pub(in crate::backend::direct_wasm) fn symbol_to_primitive_expression() -> Expression {
    Expression::Member {
        object: Box::new(Expression::Identifier("Symbol".to_string())),
        property: Box::new(Expression::String("toPrimitive".to_string())),
    }
}

pub(in crate::backend::direct_wasm) fn argument_index_from_expression(
    expression: &Expression,
) -> Option<u32> {
    match expression {
        Expression::Number(value) if value.is_finite() && value.fract() == 0.0 && *value >= 0.0 => {
            let index = *value as u64;
            (index <= u32::MAX as u64).then_some(index as u32)
        }
        Expression::String(text) => canonical_array_index_from_property_name(text),
        _ => None,
    }
}

pub(in crate::backend::direct_wasm) fn canonical_array_index_from_property_name(
    text: &str,
) -> Option<u32> {
    let index = text.parse::<u32>().ok()?;
    if index == u32::MAX || index.to_string() != text {
        return None;
    }
    Some(index)
}

pub(in crate::backend::direct_wasm) fn normalize_js_scientific_notation(text: String) -> String {
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

pub(in crate::backend::direct_wasm) fn js_number_property_name(value: f64) -> String {
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

pub(in crate::backend::direct_wasm) fn static_numeric_property_name_value(
    expression: &Expression,
) -> Option<f64> {
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

pub(in crate::backend::direct_wasm) fn static_property_name_from_expression(
    expression: &Expression,
) -> Option<String> {
    match expression {
        Expression::String(text) => Some(text.clone()),
        Expression::Bool(value) => Some(value.to_string()),
        Expression::BigInt(value) => Some(value.clone()),
        Expression::Null => Some("null".to_string()),
        Expression::Undefined => Some("undefined".to_string()),
        _ => static_numeric_property_name_value(expression).map(js_number_property_name),
    }
}

pub(in crate::backend::direct_wasm) fn hex_digit_value(character: char) -> Option<u32> {
    match character {
        '0'..='9' => Some(character as u32 - '0' as u32),
        'A'..='F' => Some(character as u32 - 'A' as u32 + 10),
        'a'..='f' => Some(character as u32 - 'a' as u32 + 10),
        _ => None,
    }
}

pub(in crate::backend::direct_wasm) fn parse_fixed_hex_quad(text: &str) -> Option<u32> {
    if text.len() != 4 {
        return None;
    }

    let mut value = 0u32;
    for character in text.chars() {
        value = (value << 4) | hex_digit_value(character)?;
    }
    Some(value)
}

pub(in crate::backend::direct_wasm) fn is_canonical_hex_digit_array(
    array_binding: &ArrayValueBinding,
) -> bool {
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

pub(in crate::backend::direct_wasm) fn enumerated_keys_from_array_binding(
    array_binding: &ArrayValueBinding,
) -> ArrayValueBinding {
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
