use super::super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_if_condition_value(
        &self,
        expression: &Expression,
    ) -> Option<bool> {
        if let Expression::Binary { op, left, right } = expression {
            let compare = |lhs: bool, rhs: bool| match op {
                BinaryOp::Equal | BinaryOp::LooseEqual => Some(lhs == rhs),
                BinaryOp::NotEqual | BinaryOp::LooseNotEqual => Some(lhs != rhs),
                _ => None,
            };
            if let Some(lhs) = self.resolve_static_is_nan_call_result(left)
                && let Expression::Bool(rhs) = self.materialize_static_expression(right)
            {
                return compare(lhs, rhs);
            }
            if let Some(rhs) = self.resolve_static_is_nan_call_result(right)
                && let Expression::Bool(lhs) = self.materialize_static_expression(left)
            {
                return compare(lhs, rhs);
            }
        }
        self.resolve_static_boolean_expression(expression)
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_logical_result_expression(
        &self,
        op: BinaryOp,
        left: &Expression,
        right: &Expression,
    ) -> Option<Expression> {
        match op {
            BinaryOp::LogicalAnd => {
                let left_truthy = self.resolve_static_boolean_expression(left)?;
                if left_truthy {
                    Some(self.materialize_static_expression(right))
                } else {
                    Some(self.materialize_static_expression(left))
                }
            }
            BinaryOp::LogicalOr => {
                let left_truthy = self.resolve_static_boolean_expression(left)?;
                if left_truthy {
                    Some(self.materialize_static_expression(left))
                } else {
                    Some(self.materialize_static_expression(right))
                }
            }
            BinaryOp::NullishCoalescing => {
                let materialized_left = self.materialize_static_expression(left);
                if let Some(primitive_left) = self.resolve_static_primitive_expression_with_context(
                    &materialized_left,
                    self.current_function_name(),
                ) {
                    return if matches!(primitive_left, Expression::Null | Expression::Undefined) {
                        Some(self.materialize_static_expression(right))
                    } else {
                        Some(primitive_left)
                    };
                }
                matches!(
                    self.infer_value_kind(&materialized_left),
                    Some(kind) if kind != StaticValueKind::Unknown
                )
                .then_some(materialized_left)
            }
            _ => None,
        }
    }
}
