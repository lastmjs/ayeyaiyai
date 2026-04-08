use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn apply_special_function_bindings(
        bindings: &mut EntryBindingState,
        user_function: Option<&UserFunction>,
        declaration: Option<&FunctionDeclaration>,
    ) {
        if let Some(user_function) = user_function
            && let Some(function) = declaration
            && let Some(binding_name) = function
                .self_binding
                .as_ref()
                .or(function.top_level_binding.as_ref())
        {
            bindings.static_bindings.local_function_bindings.insert(
                binding_name.clone(),
                LocalFunctionBinding::User(user_function.name.clone()),
            );
            bindings.static_bindings.local_value_bindings.insert(
                binding_name.clone(),
                Expression::Identifier(user_function.name.clone()),
            );
            bindings
                .static_bindings
                .local_kinds
                .insert(binding_name.clone(), StaticValueKind::Function);
        }
        if declaration.is_some_and(|function| function.derived_constructor) {
            bindings
                .static_bindings
                .local_value_bindings
                .insert("this".to_string(), Expression::Undefined);
            bindings
                .static_bindings
                .local_kinds
                .insert("this".to_string(), StaticValueKind::Undefined);
            bindings
                .static_bindings
                .local_object_bindings
                .remove("this");
        }
    }
}
