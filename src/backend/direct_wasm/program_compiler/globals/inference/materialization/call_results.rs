use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn infer_static_call_result_expression(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Option<Expression> {
        let Expression::Identifier(_) = callee else {
            return None;
        };
        let binding = self.infer_global_function_binding(callee)?;
        let user_function = match &binding {
            LocalFunctionBinding::User(function_name) => self.user_function(function_name)?,
            LocalFunctionBinding::Builtin(_) => return None,
        };
        if user_function.is_async() {
            return None;
        }

        let context = self.static_eval_context();
        execute_static_user_function_binding_in_global_maps(
            &context,
            &binding,
            arguments,
            &mut HashMap::new(),
            &mut HashMap::new(),
            StaticFunctionEffectMode::Commit,
        )
    }
}
