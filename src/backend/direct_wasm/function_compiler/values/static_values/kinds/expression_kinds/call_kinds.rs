use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn infer_call_expression_kind(
        &self,
        expression: &Expression,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Option<StaticValueKind> {
        if self
            .resolve_static_has_own_property_call_result(expression)
            .is_some()
            || self
                .resolve_static_object_is_call_result(expression)
                .is_some()
            || self
                .resolve_static_array_is_array_call_result(expression)
                .is_some()
        {
            return Some(StaticValueKind::Bool);
        }
        if arguments.is_empty()
            && let Expression::Member { object, property } = callee
            && let Expression::String(property_name) = property.as_ref()
            && let Some(StaticEvalOutcome::Value(value)) = self
                .resolve_static_member_call_outcome_with_context(
                    object,
                    property_name,
                    self.current_function_name(),
                )
        {
            return self.infer_value_kind(&value);
        }
        if let Some((value, _)) = self.resolve_static_call_result_expression_with_context(
            callee,
            arguments,
            self.current_function_name(),
        ) {
            return self.infer_value_kind(&value);
        }
        match callee {
            Expression::Identifier(name) => self
                .infer_call_result_kind(name)
                .or(Some(StaticValueKind::Unknown)),
            _ => Some(StaticValueKind::Unknown),
        }
    }
}
