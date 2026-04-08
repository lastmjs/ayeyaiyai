use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn resolve_call_or_new_object_binding(
        &self,
        expression: &Expression,
    ) -> Option<ObjectValueBinding> {
        match expression {
            Expression::Call { callee, arguments } => {
                self.resolve_call_object_binding(callee, arguments)
            }
            Expression::New { callee, arguments } => {
                self.resolve_new_object_binding(expression, callee, arguments)
            }
            _ => None,
        }
    }

    fn resolve_call_object_binding(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Option<ObjectValueBinding> {
        if matches!(
            callee,
            Expression::Member { object, property }
                if matches!(object.as_ref(), Expression::Identifier(name) if name == "Object")
                    && matches!(property.as_ref(), Expression::String(name) if name == "create")
        ) {
            return Some(empty_object_value_binding());
        }
        if arguments.is_empty()
            && matches!(
                callee,
                Expression::Member { property, .. } if is_symbol_iterator_expression(property)
            )
        {
            let Expression::Member { object, property } = callee else {
                unreachable!("filtered above");
            };
            if let Some(object_binding) = self.resolve_object_binding_from_expression(object) {
                let has_next_method = object_binding_lookup_value(
                    &object_binding,
                    &Expression::String("next".to_string()),
                )
                .and_then(|value| self.resolve_function_binding_from_expression(value))
                .is_some();
                if self
                    .resolve_member_function_binding(object, property)
                    .is_some()
                    || self
                        .resolve_member_getter_binding(object, property)
                        .is_some()
                    || self.resolve_iterator_source_kind(object).is_some()
                    || has_next_method
                {
                    return Some(object_binding);
                }
            }
            return self
                .resolve_iterator_source_kind(object)
                .map(|_| empty_object_value_binding());
        }
        if arguments.is_empty()
            && matches!(
                callee,
                Expression::Member { object, property }
                    if matches!(object.as_ref(), Expression::Identifier(name) if name == "$262")
                        && matches!(property.as_ref(), Expression::String(name) if name == "createRealm")
            )
        {
            return Some(empty_object_value_binding());
        }
        self.resolve_native_error_object_binding(callee, arguments)
            .or_else(|| {
                self.resolve_static_call_result_expression_with_context(
                    callee,
                    arguments,
                    self.current_function_name(),
                )
                .filter(|_| matches!(callee, Expression::Identifier(_)))
                .and_then(|(result_expression, _)| {
                    self.resolve_object_binding_from_expression(&result_expression)
                })
            })
            .or_else(|| self.resolve_returned_object_binding_from_call(callee, arguments))
            .or_else(|| {
                if !arguments.is_empty() {
                    return None;
                }
                let LocalFunctionBinding::User(function_name) = self
                    .resolve_function_binding_from_expression_with_context(
                        callee,
                        self.current_function_name(),
                    )?
                else {
                    return None;
                };
                let (result_expression, _) = self
                    .execute_simple_static_user_function_with_bindings(
                        &function_name,
                        &HashMap::new(),
                    )?;
                self.resolve_object_binding_from_expression(&result_expression)
            })
    }

    fn resolve_new_object_binding(
        &self,
        expression: &Expression,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Option<ObjectValueBinding> {
        self.resolve_native_error_object_binding(callee, arguments)
            .or_else(|| self.resolve_user_constructor_new_binding(expression, callee, arguments))
            .or_else(|| {
                (arguments.is_empty()
                    && matches!(callee, Expression::Identifier(name) if name == "Object"))
                .then(empty_object_value_binding)
            })
            .or_else(|| {
                matches!(callee, Expression::Identifier(name) if name == "WeakRef")
                    .then(empty_object_value_binding)
            })
    }

    fn resolve_user_constructor_new_binding(
        &self,
        expression: &Expression,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Option<ObjectValueBinding> {
        self.resolve_user_constructor_object_binding_from_new(callee, arguments)
            .or_else(|| {
                self.state
                    .speculation
                    .static_semantics
                    .last_bound_user_function_call
                    .as_ref()
                    .filter(|snapshot| {
                        snapshot
                            .source_expression
                            .as_ref()
                            .is_some_and(|source| static_expression_matches(source, expression))
                    })
                    .and_then(|snapshot| snapshot.result_expression.as_ref())
                    .and_then(|result| self.resolve_object_binding_from_expression(result))
            })
    }

    fn resolve_native_error_object_binding(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Option<ObjectValueBinding> {
        if !matches!(
            callee,
            Expression::Identifier(name) if native_error_runtime_value(name).is_some()
        ) {
            return None;
        }

        let mut object_binding = empty_object_value_binding();
        if let Expression::Identifier(name) = callee {
            object_binding_set_property(
                &mut object_binding,
                Expression::String("name".to_string()),
                Expression::String(name.clone()),
            );
        }
        if let Some(
            CallArgument::Expression(message_expression) | CallArgument::Spread(message_expression),
        ) = arguments.get(1)
        {
            let materialized_message = self.materialize_static_expression(message_expression);
            if !matches!(materialized_message, Expression::Undefined)
                && !matches!(&materialized_message, Expression::Identifier(name)
                    if name == "undefined" && self.is_unshadowed_builtin_identifier(name))
            {
                object_binding_set_property(
                    &mut object_binding,
                    Expression::String("message".to_string()),
                    materialized_message,
                );
            }
        }
        Some(object_binding)
    }
}
