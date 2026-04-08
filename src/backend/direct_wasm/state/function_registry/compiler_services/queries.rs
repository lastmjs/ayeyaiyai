use super::super::*;

impl CompilerState {
    pub(in crate::backend::direct_wasm) fn resolve_user_function_by_binding_name(
        &self,
        name: &str,
    ) -> Option<&UserFunction> {
        if let Some(LocalFunctionBinding::User(function_name)) = self.global_function_binding(name)
        {
            return self.user_function(function_name);
        }
        if is_internal_user_function_identifier(name) {
            return self.user_function(name);
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn user_function(
        &self,
        name: &str,
    ) -> Option<&UserFunction> {
        self.function_registry.user_function(name)
    }

    pub(in crate::backend::direct_wasm) fn registered_function(
        &self,
        name: &str,
    ) -> Option<&FunctionDeclaration> {
        self.function_registry.registered_function(name)
    }

    pub(in crate::backend::direct_wasm) fn next_user_function_index(&self) -> u32 {
        self.function_registry.next_user_function_index()
    }

    pub(in crate::backend::direct_wasm) fn user_functions(&self) -> &[UserFunction] {
        self.function_registry.user_functions()
    }

    pub(in crate::backend::direct_wasm) fn user_function_parameter_bindings(
        &self,
        function_name: &str,
    ) -> PreparedFunctionParameterBindings {
        self.function_registry.parameter_bindings_for(function_name)
    }

    #[cfg(test)]
    pub(in crate::backend::direct_wasm) fn prepared_user_function_metadata_snapshot(
        &self,
    ) -> HashMap<String, PreparedFunctionMetadata> {
        self.function_registry.prepared_metadata_snapshot()
    }

    pub(in crate::backend::direct_wasm) fn user_type_arities_snapshot(&self) -> Vec<u32> {
        self.function_registry.types.user_type_arities_snapshot()
    }

    pub(in crate::backend::direct_wasm) fn contains_user_function(&self, name: &str) -> bool {
        self.function_registry.contains_user_function(name)
    }

    pub(in crate::backend::direct_wasm) fn eval_local_function_bindings_snapshot(
        &self,
    ) -> HashMap<String, HashMap<String, String>> {
        self.function_registry
            .eval_local_function_bindings_snapshot()
    }

    pub(in crate::backend::direct_wasm) fn user_function_capture_bindings_snapshot(
        &self,
    ) -> HashMap<String, HashMap<String, String>> {
        self.function_registry
            .user_function_capture_bindings_snapshot()
    }

    pub(in crate::backend::direct_wasm) fn eval_local_function_bindings(
        &self,
        function_name: &str,
    ) -> Option<&HashMap<String, String>> {
        self.function_registry
            .eval_local_function_bindings(function_name)
    }

    pub(in crate::backend::direct_wasm) fn user_function_capture_bindings(
        &self,
        function_name: &str,
    ) -> Option<&HashMap<String, String>> {
        self.function_registry
            .user_function_capture_bindings(function_name)
    }

    pub(in crate::backend::direct_wasm) fn user_function_home_object_binding(
        &self,
        function_name: &str,
    ) -> Option<String> {
        self.function_registry
            .user_function_home_object_binding(function_name)
    }
}
