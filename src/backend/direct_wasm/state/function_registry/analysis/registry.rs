use super::*;

#[derive(Default)]
pub(in crate::backend::direct_wasm) struct UserFunctionAnalysisRegistry {
    pub(in crate::backend::direct_wasm) user_function_parameter_analysis:
        UserFunctionParameterAnalysis,
    pub(in crate::backend::direct_wasm) eval_local_function_bindings:
        HashMap<String, HashMap<String, String>>,
    pub(in crate::backend::direct_wasm) user_function_capture_bindings:
        HashMap<String, HashMap<String, String>>,
}

impl UserFunctionAnalysisRegistry {
    pub(in crate::backend::direct_wasm) fn reset_for_program(&mut self) {
        self.user_function_parameter_analysis.clear();
        self.eval_local_function_bindings.clear();
        self.user_function_capture_bindings.clear();
    }

    pub(in crate::backend::direct_wasm) fn set_parameter_analysis(
        &mut self,
        analysis: UserFunctionParameterAnalysis,
    ) {
        self.user_function_parameter_analysis = analysis;
    }

    pub(in crate::backend::direct_wasm) fn parameter_bindings_for(
        &self,
        function_name: &str,
    ) -> PreparedFunctionParameterBindings {
        self.user_function_parameter_analysis
            .bindings_for(function_name)
    }

    pub(in crate::backend::direct_wasm) fn user_function_capture_bindings_snapshot(
        &self,
    ) -> HashMap<String, HashMap<String, String>> {
        self.user_function_capture_bindings.clone()
    }

    pub(in crate::backend::direct_wasm) fn eval_local_function_bindings_snapshot(
        &self,
    ) -> HashMap<String, HashMap<String, String>> {
        self.eval_local_function_bindings.clone()
    }

    pub(in crate::backend::direct_wasm) fn eval_local_function_bindings(
        &self,
        function_name: &str,
    ) -> Option<&HashMap<String, String>> {
        self.eval_local_function_bindings.get(function_name)
    }

    pub(in crate::backend::direct_wasm) fn user_function_capture_bindings(
        &self,
        function_name: &str,
    ) -> Option<&HashMap<String, String>> {
        self.user_function_capture_bindings.get(function_name)
    }

    pub(in crate::backend::direct_wasm) fn record_eval_local_function_binding(
        &mut self,
        function_name: &str,
        binding_name: &str,
        hidden_name: &str,
    ) {
        self.eval_local_function_bindings
            .entry(function_name.to_string())
            .or_default()
            .insert(binding_name.to_string(), hidden_name.to_string());
    }

    pub(in crate::backend::direct_wasm) fn clear_user_function_capture_bindings(&mut self) {
        self.user_function_capture_bindings.clear();
    }

    pub(in crate::backend::direct_wasm) fn set_user_function_capture_bindings(
        &mut self,
        function_name: &str,
        captures: HashMap<String, String>,
    ) {
        self.user_function_capture_bindings
            .insert(function_name.to_string(), captures);
    }
}
