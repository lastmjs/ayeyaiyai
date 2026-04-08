use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_member_call_outcome_with_context(
        &self,
        object: &Expression,
        property_name: &str,
        current_function_name: Option<&str>,
    ) -> Option<StaticEvalOutcome> {
        let property = Expression::String(property_name.to_string());
        if let Some(object_binding) = self.resolve_object_binding_from_expression(object)
            && let Some(method_value) = object_binding_lookup_value(&object_binding, &property)
        {
            let binding = self.resolve_function_binding_from_expression_with_context(
                method_value,
                current_function_name,
            )?;
            return self.resolve_static_function_outcome_from_binding_with_context(
                &binding,
                &[],
                current_function_name,
            );
        }

        if let Some(value) = self.resolve_static_boxed_primitive_value(object) {
            return match property_name {
                "valueOf" => Some(StaticEvalOutcome::Value(value)),
                "toString" => Some(StaticEvalOutcome::Value(Expression::String(
                    self.resolve_static_string_concat_value(&value, current_function_name)?,
                ))),
                _ => None,
            };
        }

        if let Some(symbol_text) =
            self.resolve_static_symbol_to_string_value_with_context(object, current_function_name)
        {
            return match property_name {
                "toString" => Some(StaticEvalOutcome::Value(Expression::String(symbol_text))),
                "valueOf" => Some(StaticEvalOutcome::Value(
                    self.resolve_bound_alias_expression(object)
                        .unwrap_or_else(|| self.materialize_static_expression(object)),
                )),
                _ => None,
            };
        }

        if let Some(timestamp) = self.resolve_static_date_timestamp(object) {
            return match property_name {
                "toString" => Some(StaticEvalOutcome::Value(Expression::String(
                    self.synthesize_static_date_string(timestamp),
                ))),
                "valueOf" => Some(StaticEvalOutcome::Value(Expression::Number(timestamp))),
                _ => None,
            };
        }

        if let Some(binding) = self
            .resolve_function_binding_from_expression_with_context(object, current_function_name)
            && let LocalFunctionBinding::User(function_name) = binding
        {
            return match property_name {
                "toString" => Some(StaticEvalOutcome::Value(Expression::String(
                    self.synthesize_static_function_to_string(&function_name),
                ))),
                _ => None,
            };
        }

        if self
            .resolve_object_binding_from_expression(object)
            .is_some()
        {
            return match property_name {
                "toString" => Some(StaticEvalOutcome::Value(Expression::String(
                    "[object Object]".to_string(),
                ))),
                _ => None,
            };
        }

        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_to_primitive_outcome_with_context(
        &self,
        expression: &Expression,
        hint: PrimitiveHint,
        current_function_name: Option<&str>,
    ) -> Option<StaticEvalOutcome> {
        let resolved = self
            .resolve_bound_alias_expression(expression)
            .unwrap_or_else(|| expression.clone());
        if let Some(primitive) =
            self.resolve_static_primitive_expression_with_context(&resolved, current_function_name)
        {
            return Some(StaticEvalOutcome::Value(primitive));
        }

        if let Some(outcome) = self.resolve_static_symbol_to_primitive_outcome_with_context(
            expression,
            current_function_name,
        ) {
            return Some(outcome);
        }
        if !static_expression_matches(&resolved, expression)
            && let Some(outcome) = self.resolve_static_symbol_to_primitive_outcome_with_context(
                &resolved,
                current_function_name,
            )
        {
            return Some(outcome);
        }
        if self.symbol_to_primitive_requires_runtime_with_context(expression, current_function_name)
            || (!static_expression_matches(&resolved, expression)
                && self.symbol_to_primitive_requires_runtime_with_context(
                    &resolved,
                    current_function_name,
                ))
        {
            return None;
        }

        let coercion_target = if matches!(expression, Expression::Identifier(_)) {
            expression
        } else {
            &resolved
        };

        if self.ordinary_to_primitive_requires_runtime_with_context(
            coercion_target,
            current_function_name,
        ) {
            return None;
        }

        let prefers_string = matches!(hint, PrimitiveHint::Default)
            && self
                .resolve_static_date_timestamp(coercion_target)
                .is_some();
        let method_order = if prefers_string {
            ["toString", "valueOf"]
        } else {
            ["valueOf", "toString"]
        };

        for method_name in method_order {
            let outcome = self.resolve_static_member_call_outcome_with_context(
                coercion_target,
                method_name,
                current_function_name,
            );
            match outcome {
                Some(StaticEvalOutcome::Value(value)) => {
                    if let Some(primitive) = self.resolve_static_primitive_expression_with_context(
                        &value,
                        current_function_name,
                    ) {
                        return Some(StaticEvalOutcome::Value(primitive));
                    }
                }
                Some(StaticEvalOutcome::Throw(throw_value)) => {
                    return Some(StaticEvalOutcome::Throw(throw_value));
                }
                None => continue,
            }
        }

        if self
            .resolve_object_binding_from_expression(coercion_target)
            .is_some()
            || self
                .resolve_static_date_timestamp(coercion_target)
                .is_some()
            || self
                .resolve_function_binding_from_expression_with_context(
                    coercion_target,
                    current_function_name,
                )
                .is_some()
        {
            return Some(StaticEvalOutcome::Throw(StaticThrowValue::NamedError(
                "TypeError",
            )));
        }

        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_addition_outcome_with_context(
        &self,
        left: &Expression,
        right: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<StaticEvalOutcome> {
        if self.expression_depends_on_active_loop_assignment(left)
            || self.expression_depends_on_active_loop_assignment(right)
        {
            return None;
        }
        if current_function_name.is_some()
            && (self.addition_operand_requires_runtime_value(left)
                || self.addition_operand_requires_runtime_value(right))
        {
            return None;
        }
        let left_primitive = self.resolve_static_to_primitive_outcome_with_context(
            left,
            PrimitiveHint::Default,
            current_function_name,
        )?;
        let right_primitive = self.resolve_static_to_primitive_outcome_with_context(
            right,
            PrimitiveHint::Default,
            current_function_name,
        )?;
        let (left_value, right_value) = match (left_primitive, right_primitive) {
            (StaticEvalOutcome::Throw(throw_value), _)
            | (_, StaticEvalOutcome::Throw(throw_value)) => {
                return Some(StaticEvalOutcome::Throw(throw_value));
            }
            (StaticEvalOutcome::Value(left_value), StaticEvalOutcome::Value(right_value)) => {
                (left_value, right_value)
            }
        };

        if self.infer_value_kind(&left_value) == Some(StaticValueKind::String)
            || self.infer_value_kind(&right_value) == Some(StaticValueKind::String)
        {
            return Some(StaticEvalOutcome::Value(Expression::String(format!(
                "{}{}",
                self.resolve_static_string_concat_value(&left_value, current_function_name)?,
                self.resolve_static_string_concat_value(&right_value, current_function_name)?,
            ))));
        }

        let left_kind = self.infer_value_kind(&left_value);
        let right_kind = self.infer_value_kind(&right_value);
        if left_kind == Some(StaticValueKind::BigInt) && right_kind == Some(StaticValueKind::BigInt)
        {
            return Some(StaticEvalOutcome::Value(Expression::BigInt(
                (self.resolve_static_bigint_value(&left_value)?
                    + self.resolve_static_bigint_value(&right_value)?)
                .to_string(),
            )));
        }
        if left_kind == Some(StaticValueKind::BigInt) || right_kind == Some(StaticValueKind::BigInt)
        {
            return Some(StaticEvalOutcome::Throw(StaticThrowValue::NamedError(
                "TypeError",
            )));
        }

        Some(StaticEvalOutcome::Value(Expression::Number(
            self.resolve_static_number_value(&left_value)?
                + self.resolve_static_number_value(&right_value)?,
        )))
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_string_addition_value_with_context(
        &self,
        left: &Expression,
        right: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<String> {
        let left_primitive = match self.resolve_static_to_primitive_outcome_with_context(
            left,
            PrimitiveHint::Default,
            current_function_name,
        )? {
            StaticEvalOutcome::Value(value) => value,
            StaticEvalOutcome::Throw(_) => return None,
        };
        let right_primitive = match self.resolve_static_to_primitive_outcome_with_context(
            right,
            PrimitiveHint::Default,
            current_function_name,
        )? {
            StaticEvalOutcome::Value(value) => value,
            StaticEvalOutcome::Throw(_) => return None,
        };

        if self.infer_value_kind(&left_primitive) != Some(StaticValueKind::String)
            && self.infer_value_kind(&right_primitive) != Some(StaticValueKind::String)
        {
            return None;
        }

        Some(format!(
            "{}{}",
            self.resolve_static_string_concat_value(&left_primitive, current_function_name)?,
            self.resolve_static_string_concat_value(&right_primitive, current_function_name)?,
        ))
    }
}
