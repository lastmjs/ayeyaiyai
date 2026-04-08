use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn user_type_index_for_arity(&mut self, arity: u32) -> u32 {
        self.state
            .function_registry
            .user_type_index_for_arity(arity)
    }

    pub(in crate::backend::direct_wasm) fn next_user_function_index(&self) -> u32 {
        self.state.next_user_function_index()
    }

    pub(in crate::backend::direct_wasm) fn register_user_function(
        &mut self,
        declaration: FunctionDeclaration,
        user_function: UserFunction,
    ) {
        self.state
            .register_user_function(declaration, user_function);
    }

    pub(in crate::backend::direct_wasm) fn user_function_returned_member_function_bindings(
        &self,
        function_name: &str,
    ) -> Vec<ReturnedMemberFunctionBinding> {
        self.user_function(function_name)
            .map(|function| function.returned_member_function_bindings.clone())
            .unwrap_or_default()
    }

    pub(in crate::backend::direct_wasm) fn user_function_home_object_binding(
        &self,
        function_name: &str,
    ) -> Option<String> {
        self.state.user_function_home_object_binding(function_name)
    }
}
