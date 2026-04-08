use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn user_function(
        &self,
        function_name: &str,
    ) -> Option<&UserFunction> {
        self.prepared_program.user_function(function_name)
    }

    pub(in crate::backend::direct_wasm) fn contains_user_function(&self, name: &str) -> bool {
        self.prepared_program.contains_user_function(name)
    }

    pub(in crate::backend::direct_wasm) fn user_functions(&self) -> Vec<UserFunction> {
        self.prepared_program.ordered_user_functions()
    }

    pub(in crate::backend::direct_wasm) fn resolve_user_function_by_binding_name(
        &self,
        name: &str,
    ) -> Option<&UserFunction> {
        self.prepared_program
            .resolve_user_function_by_binding_name(name)
    }

    pub(in crate::backend::direct_wasm) fn current_user_function(&self) -> Option<&UserFunction> {
        self.state
            .speculation
            .execution_context
            .current_user_function
            .as_ref()
    }

    pub(in crate::backend::direct_wasm) fn current_function_name(&self) -> Option<&str> {
        self.state
            .speculation
            .execution_context
            .current_user_function_name
            .as_deref()
    }

    pub(in crate::backend::direct_wasm) fn has_current_user_function(&self) -> bool {
        self.state
            .speculation
            .execution_context
            .current_user_function_name
            .is_some()
    }

    pub(in crate::backend::direct_wasm) fn current_user_function_declaration(
        &self,
    ) -> Option<&FunctionDeclaration> {
        self.state
            .speculation
            .execution_context
            .current_function_declaration
            .as_ref()
    }

    pub(in crate::backend::direct_wasm) fn user_function_runtime_value(
        &self,
        function_name: &str,
    ) -> Option<i32> {
        self.user_function(function_name)
            .map(user_function_runtime_value)
    }

    pub(in crate::backend::direct_wasm) fn prepared_function_declaration(
        &self,
        function_name: &str,
    ) -> Option<&FunctionDeclaration> {
        self.prepared_program
            .user_function_declaration(function_name)
    }

    pub(in crate::backend::direct_wasm) fn user_function_capture_bindings(
        &self,
        function_name: &str,
    ) -> Option<HashMap<String, String>> {
        self.prepared_program
            .user_function_capture_bindings(function_name)
            .cloned()
    }

    pub(in crate::backend::direct_wasm) fn eval_local_function_bindings(
        &self,
        function_name: &str,
    ) -> Option<HashMap<String, String>> {
        self.prepared_program
            .eval_local_function_bindings(function_name)
            .cloned()
    }

    pub(in crate::backend::direct_wasm) fn current_function_is_derived_constructor(&self) -> bool {
        self.state.speculation.execution_context.derived_constructor
    }

    pub(in crate::backend::direct_wasm) fn user_function_is_derived_constructor(
        &self,
        user_function: &UserFunction,
    ) -> bool {
        self.resolve_registered_function_declaration(&user_function.name)
            .is_some_and(|function| function.derived_constructor)
    }
}
