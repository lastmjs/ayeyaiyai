use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn prepared_eval_local_function_bindings(
        &self,
    ) -> HashMap<String, HashMap<String, String>> {
        self.state.eval_local_function_bindings_snapshot()
    }

    pub(in crate::backend::direct_wasm) fn prepared_user_function_capture_bindings(
        &self,
    ) -> HashMap<String, HashMap<String, String>> {
        self.state.user_function_capture_bindings_snapshot()
    }

    pub(in crate::backend::direct_wasm) fn prepared_user_function(
        &self,
        function_name: &str,
    ) -> Option<UserFunction> {
        self.state.user_function(function_name).cloned()
    }

    pub(in crate::backend::direct_wasm) fn prepared_user_function_parameter_bindings(
        &self,
        function_name: &str,
    ) -> PreparedFunctionParameterBindings {
        self.state.user_function_parameter_bindings(function_name)
    }
}
