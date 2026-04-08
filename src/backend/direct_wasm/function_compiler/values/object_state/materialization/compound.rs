use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn materialize_conditional_expression(
        &self,
        condition: &Expression,
        then_expression: &Expression,
        else_expression: &Expression,
    ) -> Expression {
        let materialized_condition = self.materialize_static_expression(condition);
        if let Some(condition_value) =
            self.resolve_static_if_condition_value(&materialized_condition)
        {
            let branch = if condition_value {
                then_expression
            } else {
                else_expression
            };
            return self.materialize_static_expression(branch);
        }
        Expression::Conditional {
            condition: Box::new(materialized_condition),
            then_expression: Box::new(self.materialize_static_expression(then_expression)),
            else_expression: Box::new(self.materialize_static_expression(else_expression)),
        }
    }

    pub(in crate::backend::direct_wasm) fn materialize_call_expression(
        &self,
        expression: &Expression,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Expression {
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
            return value;
        }
        if arguments.is_empty()
            && let Expression::Member { object, property } = callee
            && let Expression::String(property_name) = property.as_ref()
            && matches!(property_name.as_str(), "toString" | "valueOf")
            && let Some(StaticEvalOutcome::Value(value)) = self
                .resolve_static_member_call_outcome_with_context(
                    object,
                    property_name,
                    self.current_function_name(),
                )
        {
            return self.materialize_static_expression(&value);
        }
        if matches!(callee, Expression::Identifier(_))
            && !self
                .resolve_user_function_from_expression(callee)
                .is_some_and(|user_function| user_function.is_async())
            && let Some(value) = self.resolve_static_call_result_expression(callee, arguments)
        {
            return self.materialize_static_expression(&value);
        }
        materialize_recursive_expression(expression, true, true, &|nested| {
            Some(self.materialize_static_expression(nested))
        })
        .expect("function-side recursive materialization supports generic call rebuild")
    }

    pub(in crate::backend::direct_wasm) fn materialize_recursive_expression_default(
        &self,
        expression: &Expression,
    ) -> Expression {
        materialize_recursive_expression(expression, true, true, &|nested| {
            Some(self.materialize_static_expression(nested))
        })
        .unwrap_or_else(|| expression.clone())
    }
}
