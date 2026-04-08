use super::*;

#[path = "booleans/builtin_calls.rs"]
mod builtin_calls;
#[path = "booleans/comparisons.rs"]
mod comparisons;
#[path = "booleans/logical_ops.rs"]
mod logical_ops;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_boolean_expression(
        &self,
        expression: &Expression,
    ) -> Option<bool> {
        let materialized = self.materialize_static_expression(expression);
        match materialized {
            Expression::Bool(value) => Some(value),
            Expression::Null | Expression::Undefined => Some(false),
            Expression::String(text) => Some(!text.is_empty()),
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                let branch = if self.resolve_static_if_condition_value(&condition)? {
                    &then_expression
                } else {
                    &else_expression
                };
                self.resolve_static_boolean_expression(branch)
            }
            Expression::Array(_)
            | Expression::Object(_)
            | Expression::New { .. }
            | Expression::This => Some(true),
            Expression::Identifier(name) => match name.as_str() {
                "undefined" => Some(false),
                "NaN" if self.is_unshadowed_builtin_identifier(name.as_str()) => Some(false),
                _ => None,
            },
            Expression::Unary {
                op: UnaryOp::Not,
                expression,
            } => Some(!self.resolve_static_boolean_expression(&expression)?),
            Expression::Binary { op, left, right } => match op {
                BinaryOp::LogicalAnd | BinaryOp::LogicalOr | BinaryOp::NullishCoalescing => self
                    .resolve_static_logical_result_expression(op, &left, &right)
                    .and_then(|value| self.resolve_static_boolean_expression(&value)),
                BinaryOp::Equal
                | BinaryOp::LooseEqual
                | BinaryOp::NotEqual
                | BinaryOp::LooseNotEqual
                | BinaryOp::LessThan
                | BinaryOp::LessThanOrEqual
                | BinaryOp::GreaterThan
                | BinaryOp::GreaterThanOrEqual => {
                    self.resolve_static_binary_boolean_result(&op, &left, &right)
                }
                _ => None,
            },
            Expression::Unary {
                op: UnaryOp::Plus,
                expression,
            }
            | Expression::Unary {
                op: UnaryOp::Negate,
                expression,
            } => {
                let number = self.resolve_static_number_value(&expression)?;
                Some(number != 0.0 && !number.is_nan())
            }
            Expression::Number(value) => Some(value != 0.0 && !value.is_nan()),
            Expression::Call { .. } => self
                .resolve_static_has_own_property_call_result(expression)
                .or_else(|| self.resolve_static_is_nan_call_result(expression))
                .or_else(|| self.resolve_static_object_is_call_result(expression))
                .or_else(|| self.resolve_static_array_is_array_call_result(expression)),
            Expression::Assign { value, .. } => self.resolve_static_boolean_expression(&value),
            _ => None,
        }
    }
}
