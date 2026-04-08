use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_string_value(
        &self,
        expression: &Expression,
    ) -> Option<String> {
        self.resolve_static_string_value_with_context(expression, self.current_function_name())
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_string_value_with_context(
        &self,
        expression: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<String> {
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self
                .resolve_static_string_value_with_context(&materialized, current_function_name);
        }
        match expression {
            Expression::String(text) => Some(text.clone()),
            Expression::BigInt(value) => Some(parse_static_bigint_literal(value)?.to_string()),
            Expression::Unary {
                op: UnaryOp::Negate,
                ..
            } => Some(self.resolve_static_bigint_value(expression)?.to_string()),
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
                self.resolve_static_string_value_with_context(branch, current_function_name)
            }
            Expression::Identifier(_) => self
                .resolve_bound_alias_expression(expression)
                .filter(|resolved| !static_expression_matches(resolved, expression))
                .or_else(|| {
                    self.resolve_global_value_expression(expression)
                        .filter(|resolved| !static_expression_matches(resolved, expression))
                })
                .and_then(|resolved| {
                    self.resolve_static_string_value_with_context(&resolved, current_function_name)
                }),
            Expression::Binary {
                op: BinaryOp::Add,
                left,
                right,
            } => {
                if let Some(StaticEvalOutcome::Value(value)) = self
                    .resolve_static_addition_outcome_with_context(
                        left,
                        right,
                        current_function_name,
                    )
                {
                    return self
                        .resolve_static_string_value_with_context(&value, current_function_name);
                }
                let left_is_string = self.infer_value_kind(left) == Some(StaticValueKind::String);
                let right_is_string = self.infer_value_kind(right) == Some(StaticValueKind::String);
                if !left_is_string && !right_is_string {
                    return None;
                }
                Some(format!(
                    "{}{}",
                    self.resolve_static_string_concat_value(left, current_function_name)?,
                    self.resolve_static_string_concat_value(right, current_function_name)?
                ))
            }
            Expression::Binary { op, left, right }
                if matches!(
                    op,
                    BinaryOp::LogicalAnd | BinaryOp::LogicalOr | BinaryOp::NullishCoalescing
                ) =>
            {
                let resolved = self.resolve_static_logical_result_expression(*op, left, right)?;
                self.resolve_static_string_value_with_context(&resolved, current_function_name)
            }
            Expression::Member { object, property } => {
                if let Some(function_name) = self.resolve_function_name_value(object, property) {
                    return Some(function_name);
                }
                if let Some(value) = self.resolve_static_member_getter_value_with_context(
                    object,
                    property,
                    current_function_name,
                ) {
                    return self
                        .resolve_static_string_value_with_context(&value, current_function_name);
                }
                if let Some(array_binding) = self.resolve_array_binding_from_expression(object) {
                    let index = argument_index_from_expression(property)? as usize;
                    return array_binding
                        .values
                        .get(index)
                        .and_then(|value: &Option<Expression>| value.as_ref())
                        .and_then(|value| {
                            self.resolve_static_string_value_with_context(
                                value,
                                current_function_name,
                            )
                        });
                }
                if let Some(object_binding) = self.resolve_object_binding_from_expression(object) {
                    let materialized_property = self.materialize_static_expression(property);
                    return object_binding_lookup_value(&object_binding, &materialized_property)
                        .and_then(|value| {
                            self.resolve_static_string_value_with_context(
                                value,
                                current_function_name,
                            )
                        });
                }
                if let Expression::String(text) = object.as_ref() {
                    let index = argument_index_from_expression(property)? as usize;
                    return text
                        .chars()
                        .nth(index)
                        .map(|character| character.to_string());
                }
                None
            }
            Expression::Call { callee, arguments } => {
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
                    return self
                        .resolve_static_string_value_with_context(&value, current_function_name);
                }
                if let Some((value, callee_function_name)) = self
                    .resolve_static_call_result_expression_with_context(
                        callee,
                        arguments,
                        current_function_name,
                    )
                {
                    return self.resolve_static_string_value_with_context(
                        &value,
                        callee_function_name.as_deref(),
                    );
                }
                let Expression::Member { object, property } = callee.as_ref() else {
                    return None;
                };
                if !matches!(object.as_ref(), Expression::Identifier(name) if name == "String") {
                    return None;
                }
                if !matches!(property.as_ref(), Expression::String(name) if name == "fromCharCode")
                {
                    return None;
                }
                let [CallArgument::Expression(argument)] = arguments.as_slice() else {
                    return None;
                };
                let Expression::Number(codepoint) = self.resolve_char_code_argument(argument)?
                else {
                    return None;
                };
                char::from_u32(codepoint as u32).map(|character| character.to_string())
            }
            _ => None,
        }
    }
}
