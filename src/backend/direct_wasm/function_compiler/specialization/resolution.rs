use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_specialized_function_value_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<SpecializedFunctionValue> {
        match expression {
            Expression::Call { callee, arguments } | Expression::New { callee, arguments } => self
                .resolve_specialized_function_value_from_returned_call_expression(
                    callee, arguments,
                ),
            Expression::Member { object, property } => self
                .resolve_specialized_function_value_from_member_getter_expression(object, property),
            Expression::Identifier(name) => self
                .state
                .speculation
                .static_semantics
                .values
                .local_specialized_function_values
                .get(name)
                .cloned()
                .or_else(|| {
                    self.backend
                        .global_semantics
                        .functions
                        .specialized_function_values
                        .get(name)
                        .cloned()
                }),
            _ => None,
        }
    }

    fn resolve_specialized_function_value_from_member_getter_expression(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<SpecializedFunctionValue> {
        let getter_binding = self.resolve_member_getter_binding(object, property)?;
        let returned_expression = self
            .resolve_function_binding_static_return_expression_with_call_frame(
                &getter_binding,
                &[],
                object,
            )?;
        let template = self
            .resolve_function_value_template_from_expression(&returned_expression)
            .or_else(|| {
                self.resolve_specialized_function_value_from_expression(&returned_expression)
            })?;
        let capture_slots = self
            .resolve_member_function_capture_slots(object, property)
            .unwrap_or_default();
        if capture_slots.is_empty() {
            return Some(template);
        }
        let captured = match &template.binding {
            LocalFunctionBinding::User(function_name) => self
                .backend
                .function_registry
                .analysis
                .user_function_capture_bindings
                .get(function_name)
                .map(|captures| captures.keys().cloned().collect::<BTreeSet<_>>())
                .unwrap_or_else(|| self.collect_capture_bindings_from_summary(&template.summary)),
            LocalFunctionBinding::Builtin(_) => {
                self.collect_capture_bindings_from_summary(&template.summary)
            }
        };
        if captured.is_empty() {
            return Some(template);
        }
        let mut bindings = HashMap::new();
        for capture_name in captured {
            let slot_name = capture_slots.get(&capture_name)?;
            bindings.insert(capture_name, Expression::Identifier(slot_name.clone()));
        }
        Some(SpecializedFunctionValue {
            binding: template.binding.clone(),
            summary: rewrite_inline_function_summary_bindings(&template.summary, &bindings),
        })
    }

    fn resolve_specialized_function_value_from_returned_call_expression(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Option<SpecializedFunctionValue> {
        let LocalFunctionBinding::User(outer_function_name) = self
            .resolve_function_binding_from_expression_with_context(
                callee,
                self.current_function_name(),
            )?
        else {
            return None;
        };
        let outer_user_function = self.user_function(&outer_function_name)?;
        let outer_function =
            self.resolve_registered_function_declaration(&outer_user_function.name)?;
        let returned_function_name = collect_returned_identifier(&outer_function.body)?;
        let inner_user_function = self.user_function(&returned_function_name)?;
        if inner_user_function.is_async()
            || inner_user_function.is_generator()
            || inner_user_function.has_parameter_defaults()
            || !inner_user_function.extra_argument_indices.is_empty()
        {
            return None;
        }
        let summary = inner_user_function.inline_summary.as_ref()?;
        if inline_summary_mentions_call_frame_state(summary) && !inner_user_function.lexical_this {
            return None;
        }

        let local_aliases = collect_returned_member_local_aliases(&outer_function.body);
        let with_scope_objects = collect_returned_identifier_with_scope_objects(
            &outer_function.body,
            &returned_function_name,
        )
        .unwrap_or_default();
        let captured = self
            .backend
            .function_registry
            .analysis
            .user_function_capture_bindings
            .get(&returned_function_name)
            .map(|bindings| bindings.keys().cloned().collect::<BTreeSet<_>>())
            .unwrap_or_else(|| self.collect_capture_bindings_from_summary(summary));
        let mut bindings = HashMap::new();

        for capture_name in captured {
            let bound_expression = if let Some(alias) = local_aliases.get(&capture_name) {
                self.substitute_user_function_argument_bindings(
                    alias,
                    outer_user_function,
                    arguments,
                )
            } else if let Some(param_name) = outer_user_function.params.iter().find(|param| {
                *param == &capture_name
                    || scoped_binding_source_name(param)
                        .is_some_and(|source_name| source_name == capture_name)
            }) {
                self.substitute_user_function_argument_bindings(
                    &Expression::Identifier(param_name.clone()),
                    outer_user_function,
                    arguments,
                )
            } else if let Some(scope_expression) =
                with_scope_objects.iter().rev().find_map(|scope_object| {
                    let aliased_scope_object = resolve_returned_member_local_alias_expression(
                        scope_object,
                        &local_aliases,
                    );
                    let substituted_scope_object = self.substitute_user_function_argument_bindings(
                        &aliased_scope_object,
                        outer_user_function,
                        arguments,
                    );
                    self.scope_object_has_binding_property(&substituted_scope_object, &capture_name)
                        .then_some(substituted_scope_object)
                })
            {
                self.materialize_static_expression(&Expression::Member {
                    object: Box::new(scope_expression),
                    property: Box::new(Expression::String(capture_name.clone())),
                })
            } else {
                Expression::Identifier(capture_name.clone())
            };

            if !inline_summary_side_effect_free_expression(&bound_expression) {
                return None;
            }
            bindings.insert(capture_name, bound_expression);
        }

        Some(SpecializedFunctionValue {
            binding: LocalFunctionBinding::User(returned_function_name),
            summary: rewrite_inline_function_summary_bindings(summary, &bindings),
        })
    }

    pub(in crate::backend::direct_wasm) fn resolve_function_value_template_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<SpecializedFunctionValue> {
        let binding = self.resolve_function_binding_from_expression(expression)?;
        let LocalFunctionBinding::User(function_name) = &binding else {
            return None;
        };
        let user_function = self.user_function(function_name)?;
        if user_function.is_async()
            || user_function.is_generator()
            || user_function.has_parameter_defaults()
        {
            return None;
        }
        if !user_function.extra_argument_indices.is_empty() {
            return None;
        }
        let summary = user_function.inline_summary.as_ref()?;
        if inline_summary_mentions_call_frame_state(summary) && !user_function.lexical_this {
            return None;
        }
        Some(SpecializedFunctionValue {
            binding,
            summary: summary.clone(),
        })
    }
}
