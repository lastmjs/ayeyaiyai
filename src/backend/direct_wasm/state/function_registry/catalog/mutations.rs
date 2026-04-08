use super::*;

impl UserFunctionCatalog {
    pub(in crate::backend::direct_wasm) fn reset_for_program(&mut self) {
        self.user_functions.clear();
        self.registered_function_declarations.clear();
        self.user_function_map.clear();
    }

    pub(in crate::backend::direct_wasm) fn register(
        &mut self,
        declaration: FunctionDeclaration,
        user_function: UserFunction,
    ) {
        self.user_functions.push(user_function.clone());
        self.registered_function_declarations.push(declaration);
        self.user_function_map
            .insert(user_function.name.clone(), user_function);
    }

    pub(in crate::backend::direct_wasm) fn set_user_function_home_object_binding(
        &mut self,
        function_name: &str,
        home_object_name: &str,
    ) {
        if let Some(user_function) = self.user_function_mut(function_name) {
            user_function.home_object_binding = Some(home_object_name.to_string());
        }
    }
}
