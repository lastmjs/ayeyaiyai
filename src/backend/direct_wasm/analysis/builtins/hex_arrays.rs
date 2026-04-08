use super::*;

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
