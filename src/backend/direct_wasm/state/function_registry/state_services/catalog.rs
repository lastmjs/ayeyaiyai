use super::super::*;

impl FunctionRegistryState {
    pub(in crate::backend::direct_wasm) fn register_user_function(
        &mut self,
        declaration: FunctionDeclaration,
        user_function: UserFunction,
    ) {
        self.catalog.register(declaration, user_function);
    }

    pub(in crate::backend::direct_wasm) fn user_function(
        &self,
        name: &str,
    ) -> Option<&UserFunction> {
        self.catalog.user_function(name)
    }

    pub(in crate::backend::direct_wasm) fn user_function_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut UserFunction> {
        self.catalog.user_function_mut(name)
    }

    pub(in crate::backend::direct_wasm) fn contains_user_function(&self, name: &str) -> bool {
        self.catalog.contains_user_function(name)
    }

    pub(in crate::backend::direct_wasm) fn registered_function(
        &self,
        name: &str,
    ) -> Option<&FunctionDeclaration> {
        self.catalog.registered_function(name)
    }

    pub(in crate::backend::direct_wasm) fn user_functions(&self) -> &[UserFunction] {
        self.catalog.user_functions()
    }

    pub(in crate::backend::direct_wasm) fn user_function_home_object_binding(
        &self,
        function_name: &str,
    ) -> Option<String> {
        self.catalog
            .user_function_home_object_binding(function_name)
    }

    pub(in crate::backend::direct_wasm) fn set_user_function_home_object_binding(
        &mut self,
        function_name: &str,
        home_object_name: &str,
    ) {
        self.catalog
            .set_user_function_home_object_binding(function_name, home_object_name);
    }

    #[cfg(test)]
    pub(in crate::backend::direct_wasm) fn prepared_metadata_snapshot(
        &self,
    ) -> HashMap<String, PreparedFunctionMetadata> {
        self.catalog.prepared_metadata_snapshot()
    }
}
