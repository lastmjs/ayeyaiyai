use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_getter_value_from_binding_with_context(
        &self,
        binding: &LocalFunctionBinding,
        this_binding: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<Expression> {
        self.resolve_function_binding_static_return_expression_with_call_frame(
            binding,
            &[],
            this_binding,
        )
        .or_else(|| {
            match self.resolve_static_function_outcome_from_binding_with_context(
                binding,
                &[],
                current_function_name,
            ) {
                Some(StaticEvalOutcome::Value(value)) => Some(value),
                _ => None,
            }
        })
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_member_getter_value_with_context(
        &self,
        object: &Expression,
        property: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<Expression> {
        let getter_binding = self.resolve_member_getter_binding(object, property)?;
        self.resolve_static_getter_value_from_binding_with_context(
            &getter_binding,
            object,
            current_function_name,
        )
    }
}
