use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn resolve_call_frame_direct_binding_substitution(
        &self,
        expression: &Expression,
        user_function: &UserFunction,
        this_binding: &Expression,
        arguments_binding: &Expression,
    ) -> Option<Expression> {
        let arguments_shadowed = user_function.body_declares_arguments_binding
            || user_function.params.iter().any(|param| {
                param == "arguments"
                    || scoped_binding_source_name(param)
                        .is_some_and(|source_name| source_name == "arguments")
            });
        let self_binding_name = self
            .resolve_registered_function_declaration(&user_function.name)
            .and_then(|function| function.self_binding.as_deref());

        match expression {
            Expression::This if !user_function.lexical_this => Some(this_binding.clone()),
            Expression::Identifier(name)
                if (name == "arguments"
                    || scoped_binding_source_name(name)
                        .is_some_and(|source_name| source_name == "arguments"))
                    && !arguments_shadowed =>
            {
                Some(arguments_binding.clone())
            }
            Expression::Identifier(name)
                if self_binding_name.is_some_and(|self_binding| {
                    name == self_binding
                        || scoped_binding_source_name(name)
                            .is_some_and(|source_name| source_name == self_binding)
                }) =>
            {
                Some(Expression::Identifier(user_function.name.clone()))
            }
            _ => None,
        }
    }
}
