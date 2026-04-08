use super::super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_binary_boolean_result(
        &self,
        op: &BinaryOp,
        left: &Expression,
        right: &Expression,
    ) -> Option<bool> {
        match op {
            BinaryOp::Equal
            | BinaryOp::LooseEqual
            | BinaryOp::NotEqual
            | BinaryOp::LooseNotEqual => {
                self.resolve_static_equality_boolean_result(op, left, right)
            }
            BinaryOp::LessThan
            | BinaryOp::LessThanOrEqual
            | BinaryOp::GreaterThan
            | BinaryOp::GreaterThanOrEqual => {
                self.resolve_static_relational_boolean_result(op, left, right)
            }
            _ => None,
        }
    }

    fn resolve_static_equality_boolean_result(
        &self,
        op: &BinaryOp,
        left: &Expression,
        right: &Expression,
    ) -> Option<bool> {
        self.resolve_static_symbol_equality_boolean(op, left, right)
            .or_else(|| self.resolve_static_object_identity_boolean(op, left, right))
            .or_else(|| self.resolve_static_primitive_equality_boolean(op, left, right))
    }

    fn resolve_static_symbol_equality_boolean(
        &self,
        op: &BinaryOp,
        left: &Expression,
        right: &Expression,
    ) -> Option<bool> {
        let left_symbol = self.resolve_symbol_identity_expression(left);
        let right_symbol = self.resolve_symbol_identity_expression(right);
        let is_not_equal = matches!(op, BinaryOp::NotEqual | BinaryOp::LooseNotEqual);
        if let (Some(left_symbol), Some(right_symbol)) =
            (left_symbol.as_ref(), right_symbol.as_ref())
        {
            return Some(static_expression_matches(left_symbol, right_symbol) ^ is_not_equal);
        }
        let symbol_vs_other = (left_symbol.is_some()
            && self.resolve_static_primitive_or_object_identity(right))
            || (right_symbol.is_some() && self.resolve_static_primitive_or_object_identity(left));
        symbol_vs_other.then_some(is_not_equal)
    }

    fn resolve_static_object_identity_boolean(
        &self,
        op: &BinaryOp,
        left: &Expression,
        right: &Expression,
    ) -> Option<bool> {
        if !matches!(op, BinaryOp::Equal | BinaryOp::NotEqual) {
            return None;
        }
        let is_not_equal = matches!(op, BinaryOp::NotEqual);
        if let (Some(left_identity), Some(right_identity)) = (
            self.resolve_static_object_identity_expression(left),
            self.resolve_static_object_identity_expression(right),
        ) {
            return Some((left_identity == right_identity) ^ is_not_equal);
        }
        let object_vs_primitive = (self
            .resolve_static_object_identity_expression(left)
            .is_some()
            && self
                .resolve_static_primitive_expression_with_context(
                    right,
                    self.current_function_name(),
                )
                .is_some())
            || (self
                .resolve_static_object_identity_expression(right)
                .is_some()
                && self
                    .resolve_static_primitive_expression_with_context(
                        left,
                        self.current_function_name(),
                    )
                    .is_some());
        object_vs_primitive.then_some(is_not_equal)
    }

    fn resolve_static_primitive_equality_boolean(
        &self,
        op: &BinaryOp,
        left: &Expression,
        right: &Expression,
    ) -> Option<bool> {
        let left_primitive = self
            .resolve_static_primitive_expression_with_context(left, self.current_function_name())?;
        let right_primitive = self.resolve_static_primitive_expression_with_context(
            right,
            self.current_function_name(),
        )?;
        let is_loose = matches!(op, BinaryOp::LooseEqual | BinaryOp::LooseNotEqual);
        let is_not_equal = matches!(op, BinaryOp::NotEqual | BinaryOp::LooseNotEqual);
        let equal = match (left_primitive, right_primitive) {
            (Expression::Bool(left), Expression::Bool(right)) => Some(left == right),
            (Expression::Number(left), Expression::Number(right)) => Some(left == right),
            (Expression::String(left), Expression::String(right)) => Some(left == right),
            (Expression::Null, Expression::Null)
            | (Expression::Undefined, Expression::Undefined) => Some(true),
            (Expression::Null, Expression::Undefined)
            | (Expression::Undefined, Expression::Null)
                if is_loose =>
            {
                Some(true)
            }
            (_, _) if !is_loose => Some(false),
            _ => None,
        }?;
        Some(equal ^ is_not_equal)
    }

    fn resolve_static_relational_boolean_result(
        &self,
        op: &BinaryOp,
        left: &Expression,
        right: &Expression,
    ) -> Option<bool> {
        let (Some(left_number), Some(right_number)) = (
            self.resolve_static_number_value(left),
            self.resolve_static_number_value(right),
        ) else {
            return None;
        };
        Some(match op {
            BinaryOp::LessThan => left_number < right_number,
            BinaryOp::LessThanOrEqual => left_number <= right_number,
            BinaryOp::GreaterThan => left_number > right_number,
            BinaryOp::GreaterThanOrEqual => left_number >= right_number,
            _ => unreachable!("filtered above"),
        })
    }

    fn resolve_static_primitive_or_object_identity(&self, expression: &Expression) -> bool {
        self.resolve_static_primitive_expression_with_context(
            expression,
            self.current_function_name(),
        )
        .is_some()
            || self
                .resolve_static_object_identity_expression(expression)
                .is_some()
    }
}
