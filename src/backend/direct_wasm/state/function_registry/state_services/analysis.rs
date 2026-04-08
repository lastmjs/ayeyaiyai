use super::super::*;

impl FunctionRegistryState {
    pub(in crate::backend::direct_wasm) fn set_parameter_analysis(
        &mut self,
        analysis: UserFunctionParameterAnalysis,
    ) {
        self.analysis.set_parameter_analysis(analysis);
    }

    pub(in crate::backend::direct_wasm) fn parameter_bindings_for(
        &self,
        function_name: &str,
    ) -> PreparedFunctionParameterBindings {
        self.analysis.parameter_bindings_for(function_name)
    }

    pub(in crate::backend::direct_wasm) fn user_function_capture_bindings_snapshot(
        &self,
    ) -> HashMap<String, HashMap<String, String>> {
        self.analysis.user_function_capture_bindings_snapshot()
    }

    pub(in crate::backend::direct_wasm) fn eval_local_function_bindings_snapshot(
        &self,
    ) -> HashMap<String, HashMap<String, String>> {
        self.analysis.eval_local_function_bindings_snapshot()
    }

    pub(in crate::backend::direct_wasm) fn eval_local_function_bindings(
        &self,
        function_name: &str,
    ) -> Option<&HashMap<String, String>> {
        self.analysis.eval_local_function_bindings(function_name)
    }

    pub(in crate::backend::direct_wasm) fn user_function_capture_bindings(
        &self,
        function_name: &str,
    ) -> Option<&HashMap<String, String>> {
        self.analysis.user_function_capture_bindings(function_name)
    }

    pub(in crate::backend::direct_wasm) fn record_eval_local_function_binding(
        &mut self,
        function_name: &str,
        binding_name: &str,
        hidden_name: &str,
    ) {
        self.analysis
            .record_eval_local_function_binding(function_name, binding_name, hidden_name);
    }

    pub(in crate::backend::direct_wasm) fn clear_user_function_capture_bindings(&mut self) {
        self.analysis.clear_user_function_capture_bindings();
    }

    pub(in crate::backend::direct_wasm) fn set_user_function_capture_bindings(
        &mut self,
        function_name: &str,
        captures: HashMap<String, String>,
    ) {
        self.analysis
            .set_user_function_capture_bindings(function_name, captures);
    }
}
