use super::*;

impl UserFunctionCatalog {
    pub(in crate::backend::direct_wasm) fn user_function(
        &self,
        name: &str,
    ) -> Option<&UserFunction> {
        self.user_function_map.get(name)
    }

    pub(in crate::backend::direct_wasm) fn user_function_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut UserFunction> {
        self.user_function_map.get_mut(name)
    }

    pub(in crate::backend::direct_wasm) fn contains_user_function(&self, name: &str) -> bool {
        self.user_function_map.contains_key(name)
    }

    pub(in crate::backend::direct_wasm) fn user_functions(&self) -> &[UserFunction] {
        &self.user_functions
    }

    pub(in crate::backend::direct_wasm) fn user_function_home_object_binding(
        &self,
        function_name: &str,
    ) -> Option<String> {
        self.user_function(function_name)
            .and_then(|function| function.home_object_binding.clone())
    }

    #[cfg(test)]
    pub(in crate::backend::direct_wasm) fn prepared_metadata_snapshot(
        &self,
    ) -> HashMap<String, PreparedFunctionMetadata> {
        self.user_functions
            .iter()
            .cloned()
            .zip(self.registered_function_declarations.iter().cloned())
            .map(|(user_function, declaration)| {
                (
                    declaration.name.clone(),
                    PreparedFunctionMetadata {
                        name: declaration.name.clone(),
                        declaration,
                        user_function,
                    },
                )
            })
            .collect()
    }

    pub(in crate::backend::direct_wasm) fn registered_function(
        &self,
        name: &str,
    ) -> Option<&FunctionDeclaration> {
        self.registered_function_declarations
            .iter()
            .find(|function| function.name == name)
    }
}
