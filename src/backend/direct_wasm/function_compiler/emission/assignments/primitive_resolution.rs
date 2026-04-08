use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_primitive_expression_with_context(
        &self,
        expression: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<Expression> {
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.resolve_static_primitive_expression_with_context(
                &materialized,
                current_function_name,
            );
        }

        match expression {
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined => Some(expression.clone()),
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                let branch = if self.resolve_static_if_condition_value(condition)? {
                    then_expression
                } else {
                    else_expression
                };
                self.resolve_static_primitive_expression_with_context(branch, current_function_name)
            }
            Expression::Assign { value, .. }
            | Expression::AssignMember { value, .. }
            | Expression::AssignSuperMember { value, .. } => {
                self.resolve_static_primitive_expression_with_context(value, current_function_name)
            }
            Expression::Await(value) => {
                self.resolve_static_primitive_expression_with_context(value, current_function_name)
            }
            Expression::Identifier(name)
                if name == "undefined" && self.is_unshadowed_builtin_identifier(name) =>
            {
                Some(Expression::Undefined)
            }
            Expression::Identifier(name)
                if name == "NaN" && self.is_unshadowed_builtin_identifier(name) =>
            {
                Some(Expression::Number(f64::NAN))
            }
            Expression::Identifier(name)
                if name == "Infinity" && self.is_unshadowed_builtin_identifier(name) =>
            {
                Some(Expression::Number(f64::INFINITY))
            }
            Expression::Member { object, property } => {
                if let Some(function_name) = self.resolve_function_name_value(object, property) {
                    return Some(Expression::String(function_name));
                }
                if let Some(value) = self
                    .resolve_static_member_getter_value_with_context(
                        object,
                        property,
                        current_function_name,
                    )
                    .filter(|value| !static_expression_matches(value, expression))
                {
                    return self.resolve_static_primitive_expression_with_context(
                        &value,
                        current_function_name,
                    );
                }
                if !self.function_object_has_explicit_own_property(object, property)
                    && let Some(number) = self.resolve_static_number_value(expression)
                {
                    return Some(Expression::Number(number));
                }
                None
            }
            Expression::Unary {
                op: UnaryOp::TypeOf,
                expression,
            } => Some(Expression::String(
                self.infer_typeof_operand_kind(expression)?
                    .as_typeof_str()?
                    .to_string(),
            )),
            Expression::Unary {
                op: UnaryOp::Negate,
                expression,
            } if self.infer_value_kind(expression) == Some(StaticValueKind::BigInt) => Some(
                Expression::BigInt((-self.resolve_static_bigint_value(expression)?).to_string()),
            ),
            Expression::Binary {
                op:
                    op @ (BinaryOp::BitwiseAnd
                    | BinaryOp::BitwiseOr
                    | BinaryOp::BitwiseXor
                    | BinaryOp::LeftShift
                    | BinaryOp::RightShift),
                left,
                right,
            } if self.infer_value_kind(left) == Some(StaticValueKind::BigInt)
                && self.infer_value_kind(right) == Some(StaticValueKind::BigInt) =>
            {
                let left_value = self.resolve_static_bigint_value(left)?;
                let right_value = self.resolve_static_bigint_value(right)?;
                Some(Expression::BigInt(
                    match op {
                        BinaryOp::BitwiseAnd => left_value & right_value,
                        BinaryOp::BitwiseOr => left_value | right_value,
                        BinaryOp::BitwiseXor => left_value ^ right_value,
                        BinaryOp::LeftShift => {
                            let shift = i64::try_from(right_value).ok()?;
                            if shift >= 0 {
                                left_value << usize::try_from(shift).ok()?
                            } else {
                                left_value >> usize::try_from(-shift).ok()?
                            }
                        }
                        BinaryOp::RightShift => {
                            let shift = i64::try_from(right_value).ok()?;
                            if shift >= 0 {
                                left_value >> usize::try_from(shift).ok()?
                            } else {
                                left_value << usize::try_from(-shift).ok()?
                            }
                        }
                        _ => unreachable!("filtered above"),
                    }
                    .to_string(),
                ))
            }
            Expression::Unary {
                op: UnaryOp::Plus | UnaryOp::Negate,
                ..
            }
            | Expression::Binary {
                op:
                    BinaryOp::Subtract
                    | BinaryOp::Multiply
                    | BinaryOp::Divide
                    | BinaryOp::Modulo
                    | BinaryOp::Exponentiate
                    | BinaryOp::BitwiseAnd
                    | BinaryOp::BitwiseOr
                    | BinaryOp::BitwiseXor
                    | BinaryOp::LeftShift
                    | BinaryOp::RightShift
                    | BinaryOp::UnsignedRightShift,
                ..
            } => Some(Expression::Number(
                self.resolve_static_number_value(expression)?,
            )),
            Expression::Binary {
                op: op @ (BinaryOp::LogicalAnd | BinaryOp::LogicalOr | BinaryOp::NullishCoalescing),
                left,
                right,
            } => {
                let value = self.resolve_static_logical_result_expression(*op, left, right)?;
                self.resolve_static_primitive_expression_with_context(&value, current_function_name)
            }
            Expression::Binary {
                op:
                    BinaryOp::LessThan
                    | BinaryOp::LessThanOrEqual
                    | BinaryOp::GreaterThan
                    | BinaryOp::GreaterThanOrEqual
                    | BinaryOp::Equal
                    | BinaryOp::NotEqual
                    | BinaryOp::LooseEqual
                    | BinaryOp::LooseNotEqual
                    | BinaryOp::In
                    | BinaryOp::InstanceOf,
                ..
            }
            | Expression::Unary {
                op: UnaryOp::Not | UnaryOp::Delete,
                ..
            } => Some(Expression::Bool(
                self.resolve_static_boolean_expression(expression)?,
            )),
            Expression::Binary {
                op: BinaryOp::Add,
                left,
                right,
            } => match self.resolve_static_addition_outcome_with_context(
                left,
                right,
                current_function_name,
            )? {
                StaticEvalOutcome::Value(value) => self
                    .resolve_static_primitive_expression_with_context(
                        &value,
                        current_function_name,
                    ),
                StaticEvalOutcome::Throw(_) => None,
            },
            Expression::Call { callee, arguments } => {
                if let Some(value) = self
                    .resolve_static_has_own_property_call_result(expression)
                    .map(Expression::Bool)
                    .or_else(|| {
                        self.resolve_static_is_nan_call_result(expression)
                            .map(Expression::Bool)
                    })
                    .or_else(|| {
                        self.resolve_static_object_is_call_result(expression)
                            .map(Expression::Bool)
                    })
                    .or_else(|| {
                        self.resolve_static_array_is_array_call_result(expression)
                            .map(Expression::Bool)
                    })
                {
                    return Some(value);
                }
                if arguments.is_empty()
                    && let Expression::Member { object, property } = callee.as_ref()
                    && let Expression::String(property_name) = property.as_ref()
                    && let Some(StaticEvalOutcome::Value(value)) = self
                        .resolve_static_member_call_outcome_with_context(
                            object,
                            property_name,
                            current_function_name,
                        )
                {
                    return self.resolve_static_primitive_expression_with_context(
                        &value,
                        current_function_name,
                    );
                }
                let (value, callee_function_name) = self
                    .resolve_static_call_result_expression_with_context(
                        callee,
                        arguments,
                        current_function_name,
                    )?;
                self.resolve_static_primitive_expression_with_context(
                    &value,
                    callee_function_name.as_deref().or(current_function_name),
                )
            }
            _ => None,
        }
    }
}
