use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_object_prototype_expression(
        &self,
        expression: &Expression,
    ) -> Option<Expression> {
        if matches!(expression, Expression::This)
            && let Some(current_function_name) = self.current_function_name()
            && current_function_name.starts_with("__ayy_class_ctor_")
            && let Some(function) = self.current_user_function_declaration()
            && let Some(self_binding) = function.self_binding.as_deref()
        {
            return Some(Self::prototype_member_expression(self_binding));
        }
        if self.expression_is_known_array_value(expression) {
            return Some(Self::prototype_member_expression("Array"));
        }
        if self.expression_is_known_promise_instance_for_instanceof(expression) {
            return Some(Self::prototype_member_expression("Promise"));
        }
        let preserve_tracked_expression = match expression {
            Expression::Identifier(name) => self.backend.global_has_prototype_object_binding(name),
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "prototype") => {
                match object.as_ref() {
                    Expression::Identifier(name) => self
                        .state
                        .speculation
                        .static_semantics
                        .local_object_binding(name)
                        .or_else(|| self.backend.global_object_binding(name))
                        .and_then(|object_binding| {
                            object_binding_lookup_value(
                                object_binding,
                                &Expression::String("prototype".to_string()),
                            )
                        })
                        .is_some(),
                    _ => false,
                }
            }
            _ => false,
        };
        if !preserve_tracked_expression
            && let Some(resolved) = self
                .resolve_bound_alias_expression(expression)
                .filter(|resolved| !static_expression_matches(resolved, expression))
        {
            return self.resolve_static_object_prototype_expression(&resolved);
        }

        match expression {
            Expression::Sequence(expressions) => {
                let last = expressions.last()?;
                return self.resolve_static_object_prototype_expression(last);
            }
            Expression::Identifier(name) => {
                if let Some(prototype) = self.global_object_prototype_expression(name) {
                    return Some(prototype.clone());
                }
                if let Some(value) = self
                    .state
                    .speculation
                    .static_semantics
                    .local_value_binding(name)
                    .or_else(|| self.global_value_binding(name))
                    .filter(
                        |value| !matches!(value, Expression::Identifier(alias) if alias == name),
                    )
                    && let Some(prototype) = self.resolve_static_object_prototype_expression(value)
                {
                    return Some(prototype);
                }
                if let Some(prototype) = Self::builtin_constructor_object_prototype_expression(name)
                {
                    return Some(prototype);
                }
                if self
                    .resolve_function_binding_from_expression(expression)
                    .is_some()
                {
                    return Some(Self::prototype_member_expression("Function"));
                }
            }
            Expression::Object(_) => {
                return Some(
                    object_literal_prototype_expression(expression)
                        .unwrap_or_else(|| Self::prototype_member_expression("Object")),
                );
            }
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "prototype") =>
            {
                let Expression::Identifier(name) = object.as_ref() else {
                    return None;
                };
                if let Some(value) = self
                    .state
                    .speculation
                    .static_semantics
                    .local_object_binding(name)
                    .or_else(|| self.global_object_binding(name))
                    .and_then(|object_binding| {
                        object_binding_lookup_value(
                            object_binding,
                            &Expression::String("prototype".to_string()),
                        )
                    })
                    .cloned()
                    && let Some(prototype) = self.resolve_static_object_prototype_expression(&value)
                {
                    return Some(prototype);
                }
                if let Some(prototype) = Self::builtin_prototype_object_prototype_expression(name) {
                    return Some(prototype);
                }
                if self
                    .resolve_function_binding_from_expression(object)
                    .is_some()
                    || matches!(infer_call_result_kind(name), Some(_))
                {
                    return Some(Self::prototype_member_expression("Object"));
                }
            }
            Expression::New { callee, .. } => {
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                return Some(Self::prototype_member_expression(name));
            }
            Expression::Call { callee, .. } => {
                if let Expression::Member { object, property } = callee.as_ref()
                    && matches!(object.as_ref(), Expression::Identifier(name) if name == "Object")
                    && matches!(property.as_ref(), Expression::String(name) if name == "create")
                {
                    let Expression::Call { arguments, .. } = expression else {
                        unreachable!("matched expression call above");
                    };
                    if let Some(
                        CallArgument::Expression(prototype) | CallArgument::Spread(prototype),
                    ) = arguments.first()
                    {
                        let prototype = self
                            .resolve_bound_alias_expression(prototype)
                            .filter(|resolved| !static_expression_matches(resolved, prototype))
                            .unwrap_or_else(|| self.materialize_static_expression(prototype));
                        return Some(Self::normalize_static_object_prototype_target_expression(
                            &prototype,
                        ));
                    }
                }
                if matches!(
                    callee.as_ref(),
                    Expression::Member { object, property }
                        if matches!(object.as_ref(), Expression::Identifier(name) if name == "Promise")
                            && matches!(property.as_ref(), Expression::String(name) if name == "resolve")
                ) {
                    return Some(Self::prototype_member_expression("Promise"));
                }
                if self
                    .resolve_user_function_from_expression(callee.as_ref())
                    .is_some_and(|user_function| user_function.is_async())
                {
                    return Some(Self::prototype_member_expression("Promise"));
                }
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                if native_error_runtime_value(name).is_some() {
                    return Some(Self::prototype_member_expression(name));
                }
            }
            _ => {}
        }

        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.resolve_static_object_prototype_expression(&materialized);
        }
        None
    }
}
