use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn sync_derived_constructor_this_binding_after_super_call(
        &mut self,
        user_function: &UserFunction,
        arguments: &[CallArgument],
    ) {
        let this_expression = Expression::New {
            callee: Box::new(Expression::Identifier(user_function.name.clone())),
            arguments: arguments.to_vec(),
        };
        self.state
            .speculation
            .static_semantics
            .set_local_value_binding("this", this_expression.clone());
        self.state
            .speculation
            .static_semantics
            .set_local_kind("this", StaticValueKind::Object);
        let this_binding = self
            .resolve_user_constructor_object_binding_from_new(
                &Expression::Identifier(user_function.name.clone()),
                arguments,
            )
            .or_else(|| {
                self.resolve_user_constructor_object_binding_for_function(
                    user_function,
                    arguments,
                    None,
                )
            });
        if let Some(this_binding) = this_binding {
            self.state
                .speculation
                .static_semantics
                .set_local_object_binding("this", this_binding);
        }
    }

    pub(super) fn sync_derived_constructor_this_binding_after_builtin_super_call(&mut self) {
        self.state
            .speculation
            .static_semantics
            .set_local_value_binding("this", Expression::Object(Vec::new()));
        self.state
            .speculation
            .static_semantics
            .set_local_kind("this", StaticValueKind::Object);
        self.state
            .speculation
            .static_semantics
            .clear_local_object_binding("this");
    }
}
