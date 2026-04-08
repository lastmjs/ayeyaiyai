use super::*;

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
