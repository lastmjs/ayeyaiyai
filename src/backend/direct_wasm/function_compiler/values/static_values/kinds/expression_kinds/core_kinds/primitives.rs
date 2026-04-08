use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn infer_primitive_expression_kind(
        &self,
        expression: &Expression,
    ) -> Option<StaticValueKind> {
        match expression {
            Expression::Number(_) => Some(StaticValueKind::Number),
            Expression::BigInt(_) => Some(StaticValueKind::BigInt),
            Expression::String(_) => Some(StaticValueKind::String),
            Expression::Bool(_) => Some(StaticValueKind::Bool),
            Expression::Null => Some(StaticValueKind::Null),
            Expression::Undefined => Some(StaticValueKind::Undefined),
            Expression::Identifier(name) => Some(
                if name == "undefined" && self.is_unshadowed_builtin_identifier(name) {
                    StaticValueKind::Undefined
                } else if name == "NaN" && self.is_unshadowed_builtin_identifier(name) {
                    StaticValueKind::Number
                } else {
                    self.lookup_identifier_kind(name)
                        .unwrap_or(StaticValueKind::Unknown)
                },
            ),
            Expression::Unary { op, expression } => match op {
                UnaryOp::Void => Some(StaticValueKind::Undefined),
                UnaryOp::Plus => Some(StaticValueKind::Number),
                UnaryOp::Negate => {
                    if self.infer_value_kind(expression) == Some(StaticValueKind::BigInt) {
                        Some(StaticValueKind::BigInt)
                    } else {
                        Some(StaticValueKind::Number)
                    }
                }
                UnaryOp::Not => Some(StaticValueKind::Bool),
                UnaryOp::BitwiseNot => Some(StaticValueKind::Number),
                UnaryOp::TypeOf => Some(StaticValueKind::String),
                UnaryOp::Delete => Some(StaticValueKind::Bool),
            },
            Expression::Binary { op, left, right } => match op {
                BinaryOp::Add => {
                    if let Some(StaticEvalOutcome::Value(value)) = self
                        .resolve_static_addition_outcome_with_context(
                            left,
                            right,
                            self.current_function_name(),
                        )
                    {
                        return self.infer_value_kind(&value);
                    }
                    Some(StaticValueKind::Number)
                }
                BinaryOp::Subtract
                | BinaryOp::Multiply
                | BinaryOp::Divide
                | BinaryOp::Modulo
                | BinaryOp::Exponentiate => Some(StaticValueKind::Number),
                BinaryOp::BitwiseAnd
                | BinaryOp::BitwiseOr
                | BinaryOp::BitwiseXor
                | BinaryOp::LeftShift
                | BinaryOp::RightShift => {
                    if self.infer_value_kind(left) == Some(StaticValueKind::BigInt)
                        && self.infer_value_kind(right) == Some(StaticValueKind::BigInt)
                    {
                        Some(StaticValueKind::BigInt)
                    } else {
                        Some(StaticValueKind::Number)
                    }
                }
                BinaryOp::UnsignedRightShift => Some(StaticValueKind::Number),
                BinaryOp::LessThan
                | BinaryOp::LessThanOrEqual
                | BinaryOp::GreaterThan
                | BinaryOp::GreaterThanOrEqual
                | BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::In
                | BinaryOp::InstanceOf
                | BinaryOp::LooseEqual
                | BinaryOp::LooseNotEqual => Some(StaticValueKind::Bool),
                BinaryOp::LogicalAnd | BinaryOp::LogicalOr | BinaryOp::NullishCoalescing => {
                    let left_kind = self.infer_value_kind(left);
                    let right_kind = self.infer_value_kind(right);
                    if left_kind == right_kind {
                        left_kind
                    } else {
                        Some(StaticValueKind::Unknown)
                    }
                }
            },
            Expression::Conditional {
                then_expression,
                else_expression,
                ..
            } => {
                let then_kind = self.infer_value_kind(then_expression);
                let else_kind = self.infer_value_kind(else_expression);
                if then_kind == else_kind {
                    then_kind
                } else {
                    Some(StaticValueKind::Unknown)
                }
            }
            _ => None,
        }
    }
}
