use super::super::*;

impl CompilerState {
    pub(in crate::backend::direct_wasm) fn set_user_function_parameter_analysis(
        &mut self,
        analysis: UserFunctionParameterAnalysis,
    ) {
        self.function_registry.set_parameter_analysis(analysis);
    }

    pub(in crate::backend::direct_wasm) fn register_user_function(
        &mut self,
        declaration: FunctionDeclaration,
        user_function: UserFunction,
    ) {
        self.function_registry
            .register_user_function(declaration, user_function);
    }

    pub(in crate::backend::direct_wasm) fn record_eval_local_function_binding(
        &mut self,
        function_name: &str,
        binding_name: &str,
        hidden_name: &str,
    ) {
        self.function_registry.record_eval_local_function_binding(
            function_name,
            binding_name,
            hidden_name,
        );
    }

    pub(in crate::backend::direct_wasm) fn clear_user_function_capture_bindings(&mut self) {
        self.function_registry
            .clear_user_function_capture_bindings();
    }

    pub(in crate::backend::direct_wasm) fn set_user_function_capture_bindings(
        &mut self,
        function_name: &str,
        captures: HashMap<String, String>,
    ) {
        self.function_registry
            .set_user_function_capture_bindings(function_name, captures);
    }

    pub(in crate::backend::direct_wasm) fn set_user_function_home_object_binding(
        &mut self,
        function_name: &str,
        home_object_name: &str,
    ) {
        self.function_registry
            .set_user_function_home_object_binding(function_name, home_object_name);
    }
}
