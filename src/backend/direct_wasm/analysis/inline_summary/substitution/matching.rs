use super::super::*;

pub(in crate::backend::direct_wasm) fn static_expression_matches(
    lhs: &Expression,
    rhs: &Expression,
) -> bool {
    match (lhs, rhs) {
        (Expression::Number(left), Expression::Number(right)) => {
            (left.is_nan() && right.is_nan()) || left == right
        }
        _ => lhs == rhs,
    }
}
