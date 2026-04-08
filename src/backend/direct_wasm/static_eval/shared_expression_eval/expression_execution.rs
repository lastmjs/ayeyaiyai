use super::*;

pub(in crate::backend::direct_wasm) fn evaluate_shared_static_expression<
    Executor: StaticBindingMutationExecutor + StaticExpressionEvaluation + ?Sized,
>(
    executor: &Executor,
    expression: &Expression,
    environment: &mut Executor::Environment,
) -> Option<Expression> {
    match expression {
        Expression::Assign { name, value } => {
            let value = executor
                .evaluate_expression(value, environment)
                .or_else(|| executor.materialize_expression(value, environment))?;
            executor.assign_binding_value(name, value.clone(), environment)?;
            Some(value)
        }
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            let property = executor
                .evaluate_expression(property, environment)
                .or_else(|| executor.materialize_expression(property, environment))?;
            let value = executor
                .evaluate_expression(value, environment)
                .or_else(|| executor.materialize_expression(value, environment))?;
            executor.assign_member_binding_value(object, property, value.clone(), environment)?;
            Some(value)
        }
        Expression::Unary {
            op: UnaryOp::Delete,
            expression,
        } => match expression.as_ref() {
            Expression::Member { object, property } => {
                let property = executor
                    .evaluate_expression(property, environment)
                    .or_else(|| executor.materialize_expression(property, environment))?;
                executor.delete_member_property(object, property, environment)?;
                Some(Expression::Bool(true))
            }
            _ => Some(Expression::Bool(true)),
        },
        Expression::Update { name, op, prefix } => {
            let current = executor
                .lookup_binding_value(name, environment)
                .unwrap_or(Expression::Undefined);
            let current_number = match current {
                Expression::Number(value) => value,
                Expression::Bool(true) => 1.0,
                Expression::Bool(false) | Expression::Null => 0.0,
                Expression::Undefined => f64::NAN,
                _ => return None,
            };
            let next_number = match op {
                UpdateOp::Increment => current_number + 1.0,
                UpdateOp::Decrement => current_number - 1.0,
            };
            let next = Expression::Number(next_number);
            executor.assign_binding_value(name, next.clone(), environment)?;
            Some(if *prefix {
                next
            } else {
                Expression::Number(current_number)
            })
        }
        Expression::Sequence(expressions) => {
            let mut last = Expression::Undefined;
            for expression in expressions {
                last = executor.evaluate_expression(expression, environment)?;
            }
            Some(last)
        }
        _ => None,
    }
}
