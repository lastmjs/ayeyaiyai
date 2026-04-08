use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_object_binding_entries_with_state(
        &self,
        entries: &[ObjectEntry],
        environment: &mut StaticResolutionEnvironment,
    ) -> Option<ObjectValueBinding> {
        let context = self.static_eval_context();
        resolve_structural_object_binding_in_environment(
            &context,
            entries,
            environment,
            &|expression, environment| {
                self.materialize_static_expression_with_state(expression, environment)
            },
            &|expression, environment| {
                self.resolve_object_binding_from_expression_with_state(expression, environment)
            },
            &|object, property, environment| {
                let binding = self.resolve_member_getter_binding(object, property)?;
                let context = self.static_eval_context();
                execute_static_user_function_binding_in_environment(
                    &context,
                    &binding,
                    &[],
                    environment,
                    StaticFunctionEffectMode::Discard,
                )
            },
        )
    }

    pub(in crate::backend::direct_wasm) fn resolve_object_binding_from_expression_with_state(
        &self,
        expression: &Expression,
        environment: &mut StaticResolutionEnvironment,
    ) -> Option<ObjectValueBinding> {
        if let Some(descriptor) =
            self.resolve_descriptor_binding_from_expression_with_state(expression, environment)
        {
            return Some(self.object_binding_from_property_descriptor(&descriptor));
        }

        resolve_stateful_object_binding_from_environment(
            expression,
            environment,
            &|expression, environment| {
                resolve_specialized_object_binding_expression(
                    expression,
                    environment,
                    |expression, _| self.resolve_array_binding_from_expression(expression),
                    |entries, environment| {
                        let mut environment = environment.fork();
                        self.resolve_object_binding_entries_with_state(entries, &mut environment)
                    },
                    |expression, environment| {
                        matches!(
                            expression,
                            Expression::Call { callee, .. }
                                if matches!(
                                    self.resolve_bound_alias_expression_with_state(
                                        callee,
                                        environment,
                                    )
                                    .as_ref(),
                                    Some(Expression::Member { object, property })
                                        if matches!(object.as_ref(), Expression::Identifier(name) if name == "Object")
                                            && matches!(property.as_ref(), Expression::String(name) if name == "create")
                                )
                        )
                    },
                    |expression, _| self.resolve_object_binding_from_expression(expression),
                )
            },
        )
    }
}
