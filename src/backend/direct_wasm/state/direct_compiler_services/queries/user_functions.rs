use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn resolve_user_function_by_binding_name(
        &self,
        name: &str,
    ) -> Option<&UserFunction> {
        self.state.resolve_user_function_by_binding_name(name)
    }

    pub(in crate::backend::direct_wasm) fn registered_function(
        &self,
        function_name: &str,
    ) -> Option<&FunctionDeclaration> {
        self.state.registered_function(function_name)
    }

    pub(in crate::backend::direct_wasm) fn user_function(
        &self,
        function_name: &str,
    ) -> Option<&UserFunction> {
        self.state.user_function(function_name)
    }

    pub(in crate::backend::direct_wasm) fn contains_user_function(&self, name: &str) -> bool {
        self.state.contains_user_function(name)
    }
}
