use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn reserve_parameter_layout(
        user_function: Option<&UserFunction>,
    ) -> (
        u32,
        u32,
        Option<u32>,
        HashMap<u32, u32>,
        bool,
        Vec<String>,
        Vec<Option<Expression>>,
    ) {
        let params = user_function
            .map(|function| function.params.as_slice())
            .unwrap_or(&[]);
        let parameter_defaults = user_function
            .map(|function| function.parameter_defaults.clone())
            .unwrap_or_default();
        let needs_parameter_scope_arguments_local = user_function.is_some_and(|function| {
            function.lexical_this
                && function.has_parameter_defaults()
                && function.body_declares_arguments_binding
        });
        let visible_param_count = params.len() as u32;
        let actual_argument_count_local = user_function
            .filter(|function| !function.lexical_this)
            .map(UserFunction::actual_argument_count_param);
        let mut extra_argument_param_locals = HashMap::new();
        let total_param_count = if let Some(user_function) = user_function {
            for index in &user_function.extra_argument_indices {
                if let Some(local_index) = user_function.extra_argument_param(*index) {
                    extra_argument_param_locals.insert(*index, local_index);
                }
            }
            user_function.wasm_param_count()
        } else {
            0
        };
        (
            visible_param_count,
            total_param_count,
            actual_argument_count_local,
            extra_argument_param_locals,
            needs_parameter_scope_arguments_local,
            params.to_vec(),
            parameter_defaults,
        )
    }
}
