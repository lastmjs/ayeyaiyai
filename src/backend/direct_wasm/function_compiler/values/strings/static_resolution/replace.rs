use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_string_replace_result_with_context(
        &self,
        source: &Expression,
        search_expression: &Expression,
        replacement_expression: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<String> {
        let source_text =
            self.resolve_static_string_value_with_context(source, current_function_name)?;
        let search_text = self
            .resolve_static_string_value_with_context(search_expression, current_function_name)?;
        let Some(match_index) = source_text.find(&search_text) else {
            return Some(source_text);
        };

        let replacement_text = if let Some(text) = self
            .resolve_static_string_value_with_context(replacement_expression, current_function_name)
        {
            text
        } else {
            let binding = self.resolve_function_binding_from_expression_with_context(
                replacement_expression,
                current_function_name,
            )?;
            let LocalFunctionBinding::User(function_name) = binding else {
                return None;
            };
            let user_function = self.user_function(&function_name)?;
            let callback_argument_expressions = vec![
                Expression::String(search_text.clone()),
                Expression::Number(match_index as f64),
                Expression::String(source_text.clone()),
            ];
            let this_binding =
                if self.should_box_sloppy_function_this(user_function, &Expression::Undefined) {
                    Expression::This
                } else {
                    Expression::Undefined
                };
            let replacement_value = self
                .resolve_function_binding_static_return_expression_with_call_frame(
                    &LocalFunctionBinding::User(function_name.clone()),
                    &callback_argument_expressions,
                    &this_binding,
                )?;
            self.resolve_static_string_value_with_context(
                &replacement_value,
                Some(function_name.as_str()),
            )?
        };

        Some(source_text.replacen(&search_text, &replacement_text, 1))
    }
}
