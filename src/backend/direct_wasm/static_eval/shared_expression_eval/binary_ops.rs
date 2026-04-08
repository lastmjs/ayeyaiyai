use super::*;

pub(in crate::backend::direct_wasm) fn evaluate_static_binary_expression<
    Executor: StaticExpressionEvaluation + ?Sized,
>(
    executor: &Executor,
    expression: &Expression,
    environment: &mut Executor::Environment,
) -> Option<Expression> {
    let Expression::Binary { op, left, right } = expression else {
        return None;
    };
    let left = executor.evaluate_expression(left, environment)?;
    let right = executor.evaluate_expression(right, environment)?;
    match op {
        BinaryOp::Add => match (&left, &right) {
            (Expression::Number(lhs), Expression::Number(rhs)) => {
                Some(Expression::Number(lhs + rhs))
            }
            (Expression::String(lhs), Expression::String(rhs)) => {
                Some(Expression::String(format!("{lhs}{rhs}")))
            }
            _ => None,
        },
        BinaryOp::Subtract => match (&left, &right) {
            (Expression::Number(lhs), Expression::Number(rhs)) => {
                Some(Expression::Number(lhs - rhs))
            }
            _ => None,
        },
        BinaryOp::Equal | BinaryOp::LooseEqual | BinaryOp::NotEqual | BinaryOp::LooseNotEqual => {
            let equal = match (&left, &right) {
                (Expression::Bool(lhs), Expression::Bool(rhs)) => lhs == rhs,
                (Expression::Number(lhs), Expression::Number(rhs)) => lhs == rhs,
                (Expression::String(lhs), Expression::String(rhs)) => lhs == rhs,
                (Expression::Null, Expression::Null)
                | (Expression::Undefined, Expression::Undefined) => true,
                (Expression::Null, Expression::Undefined)
                | (Expression::Undefined, Expression::Null)
                    if matches!(op, BinaryOp::LooseEqual | BinaryOp::LooseNotEqual) =>
                {
                    true
                }
                _ => false,
            };
            Some(Expression::Bool(match op {
                BinaryOp::Equal | BinaryOp::LooseEqual => equal,
                BinaryOp::NotEqual | BinaryOp::LooseNotEqual => !equal,
                _ => unreachable!("filtered above"),
            }))
        }
        _ => None,
    }
}
