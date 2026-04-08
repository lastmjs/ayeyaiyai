use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn with_current_user_function_name<T>(
        &mut self,
        function_name: Option<String>,
        callback: impl FnOnce(&mut Self) -> DirectResult<T>,
    ) -> DirectResult<T> {
        let previous_user_function_name = self
            .state
            .speculation
            .execution_context
            .current_user_function_name
            .clone();
        let previous_user_function = self
            .state
            .speculation
            .execution_context
            .current_user_function
            .clone();
        let previous_function_declaration = self
            .state
            .speculation
            .execution_context
            .current_function_declaration
            .clone();
        let previous_derived_constructor =
            self.state.speculation.execution_context.derived_constructor;
        let previous_arguments_callee_present = self
            .state
            .speculation
            .execution_context
            .current_arguments_callee_present;
        let previous_arguments_length_present = self
            .state
            .speculation
            .execution_context
            .current_arguments_length_present;

        let next_user_function = function_name
            .as_deref()
            .and_then(|name| self.user_function(name).cloned());
        let next_function_declaration = function_name
            .as_deref()
            .and_then(|name| self.resolve_registered_function_declaration(name).cloned());

        self.state
            .speculation
            .execution_context
            .current_user_function_name = function_name;
        self.state
            .speculation
            .execution_context
            .current_user_function = next_user_function.clone();
        self.state
            .speculation
            .execution_context
            .current_function_declaration = next_function_declaration.clone();
        self.state.speculation.execution_context.derived_constructor = next_function_declaration
            .as_ref()
            .is_some_and(|declaration| declaration.derived_constructor);
        self.state
            .speculation
            .execution_context
            .current_arguments_callee_present = next_user_function
            .as_ref()
            .is_some_and(|function| !function.lexical_this);
        self.state
            .speculation
            .execution_context
            .current_arguments_length_present = next_user_function
            .as_ref()
            .is_some_and(|function| !function.lexical_this);
        let result = callback(self);
        self.state
            .speculation
            .execution_context
            .current_user_function_name = previous_user_function_name;
        self.state
            .speculation
            .execution_context
            .current_user_function = previous_user_function;
        self.state
            .speculation
            .execution_context
            .current_function_declaration = previous_function_declaration;
        self.state.speculation.execution_context.derived_constructor = previous_derived_constructor;
        self.state
            .speculation
            .execution_context
            .current_arguments_callee_present = previous_arguments_callee_present;
        self.state
            .speculation
            .execution_context
            .current_arguments_length_present = previous_arguments_length_present;
        result
    }

    pub(in crate::backend::direct_wasm) fn with_strict_mode<T>(
        &mut self,
        strict_mode: bool,
        callback: impl FnOnce(&mut Self) -> DirectResult<T>,
    ) -> DirectResult<T> {
        let previous_strict_mode = self.state.speculation.execution_context.strict_mode;
        self.state.speculation.execution_context.strict_mode = strict_mode;
        let result = callback(self);
        self.state.speculation.execution_context.strict_mode = previous_strict_mode;
        result
    }

    pub(in crate::backend::direct_wasm) fn with_user_function_execution_context<T>(
        &mut self,
        user_function: &UserFunction,
        callback: impl FnOnce(&mut Self) -> DirectResult<T>,
    ) -> DirectResult<T> {
        let saved_execution_context = self.state.snapshot_user_function_execution_context();
        self.state
            .enter_user_function_execution_context(user_function);
        let result = callback(self);
        self.state
            .restore_user_function_execution_context(saved_execution_context);
        result
    }

    pub(in crate::backend::direct_wasm) fn with_named_function_execution_context<T>(
        &mut self,
        function_name: String,
        callback: impl FnOnce(&mut Self) -> DirectResult<T>,
    ) -> DirectResult<T> {
        if let Some(user_function) = self
            .backend
            .function_registry
            .catalog
            .user_function(&function_name)
            .cloned()
        {
            return self.with_user_function_execution_context(&user_function, callback);
        }
        self.with_current_user_function_name(Some(function_name), callback)
    }

    pub(in crate::backend::direct_wasm) fn with_scoped_lexical_bindings_cleanup<T>(
        &mut self,
        scope_names: Vec<String>,
        callback: impl FnOnce(&mut Self) -> DirectResult<T>,
    ) -> DirectResult<T> {
        let result = callback(self);
        self.pop_scoped_lexical_bindings(&scope_names);
        result
    }
}
