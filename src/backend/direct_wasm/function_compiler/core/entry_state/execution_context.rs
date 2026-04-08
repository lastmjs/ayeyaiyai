use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn prepare_execution_context(
        bindings: &EntryBindingState,
        user_function: Option<&UserFunction>,
        declaration: Option<&FunctionDeclaration>,
        strict_mode: bool,
    ) -> PreparedFunctionExecutionContext {
        let current_user_function = user_function.cloned();
        let current_function_declaration = declaration.cloned();
        let current_user_function_name = user_function.map(|function| function.name.clone());
        let current_arguments_callee_present =
            user_function.is_some_and(|function| !function.lexical_this);
        let current_arguments_length_present =
            user_function.is_some_and(|function| !function.lexical_this);
        let top_level_function = user_function.is_none();
        let derived_constructor = current_function_declaration
            .as_ref()
            .is_some_and(|declaration| declaration.derived_constructor);
        let (self_binding_local, self_binding_runtime_value) = user_function
            .and_then(|function| {
                declaration.and_then(|declaration| {
                    declaration
                        .self_binding
                        .as_ref()
                        .or(declaration.top_level_binding.as_ref())
                        .and_then(|binding_name| {
                            bindings
                                .locals
                                .get(binding_name)
                                .copied()
                                .map(|local| (local, user_function_runtime_value(function)))
                        })
                })
            })
            .map(|(local, runtime_value)| (Some(local), Some(runtime_value)))
            .unwrap_or((None, None));

        PreparedFunctionExecutionContext {
            strict_mode,
            current_user_function_name,
            current_user_function,
            current_function_declaration,
            current_arguments_callee_present,
            current_arguments_length_present,
            top_level_function,
            derived_constructor,
            self_binding_local,
            self_binding_runtime_value,
        }
    }
}
