use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn update_local_object_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Some(object_binding) = self.resolve_object_binding_from_expression(value) else {
            self.local_object_bindings.remove(name);
            if self.binding_name_is_global(name) {
                self.module.global_object_bindings.remove(name);
            }
            return;
        };
        self.local_object_bindings
            .insert(name.to_string(), object_binding.clone());
        if self.binding_name_is_global(name) {
            self.module
                .global_object_bindings
                .insert(name.to_string(), object_binding);
        }
        self.local_kinds
            .insert(name.to_string(), StaticValueKind::Object);
    }

    pub(in crate::backend::direct_wasm) fn update_prototype_object_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Some(object_binding) = self.resolve_object_binding_from_expression(value) else {
            self.local_prototype_object_bindings.remove(name);
            if self.binding_name_is_global(name) {
                self.module.global_prototype_object_bindings.remove(name);
            }
            return;
        };
        self.local_prototype_object_bindings
            .insert(name.to_string(), object_binding.clone());
        if self.binding_name_is_global(name) {
            self.module
                .global_prototype_object_bindings
                .insert(name.to_string(), object_binding);
        }
    }

    pub(in crate::backend::direct_wasm) fn update_object_binding_from_expression(
        &mut self,
        expression: &Expression,
    ) {
        let Expression::Call { callee, arguments } = expression else {
            return;
        };
        let Expression::Member { object, property } = callee.as_ref() else {
            return;
        };
        if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Object") {
            return;
        }
        if !matches!(property.as_ref(), Expression::String(name) if name == "defineProperty") {
            return;
        }
        let [
            CallArgument::Expression(target),
            CallArgument::Expression(property),
            CallArgument::Expression(descriptor_expression),
            ..,
        ] = arguments.as_slice()
        else {
            return;
        };
        let Some(descriptor) = resolve_property_descriptor_definition(descriptor_expression) else {
            return;
        };

        let update_global_property_descriptor =
            |compiler: &mut Self,
             property: &Expression,
             descriptor: &PropertyDescriptorDefinition| {
                let property = compiler
                    .resolve_property_key_expression(property)
                    .unwrap_or_else(|| compiler.materialize_static_expression(property));
                let Some(property_name) = static_property_name_from_expression(&property) else {
                    return;
                };
                let existing = compiler
                    .module
                    .global_property_descriptors
                    .get(&property_name)
                    .cloned();
                let value = if descriptor.is_accessor() {
                    Expression::Undefined
                } else {
                    descriptor
                        .value
                        .as_ref()
                        .map(|expression| compiler.materialize_static_expression(expression))
                        .or_else(|| existing.as_ref().map(|state| state.value.clone()))
                        .unwrap_or(Expression::Undefined)
                };
                let writable = if descriptor.is_accessor() {
                    None
                } else {
                    Some(
                        descriptor
                            .writable
                            .or_else(|| existing.as_ref().and_then(|state| state.writable))
                            .unwrap_or(false),
                    )
                };
                let enumerable = descriptor.enumerable.unwrap_or_else(|| {
                    existing
                        .as_ref()
                        .map(|state| state.enumerable)
                        .unwrap_or(false)
                });
                let configurable = descriptor.configurable.unwrap_or_else(|| {
                    existing
                        .as_ref()
                        .map(|state| state.configurable)
                        .unwrap_or(false)
                });
                compiler.module.global_property_descriptors.insert(
                    property_name,
                    GlobalPropertyDescriptorState {
                        value,
                        writable,
                        enumerable,
                        configurable,
                    },
                );
            };

        match target {
            Expression::This => {
                update_global_property_descriptor(self, property, &descriptor);
            }
            Expression::Identifier(name) => {
                let property = self
                    .resolve_property_key_expression(property)
                    .unwrap_or_else(|| self.materialize_static_expression(property));
                let property_name = static_property_name_from_expression(&property);
                let existing_value = self
                    .local_object_bindings
                    .get(name)
                    .or_else(|| self.module.global_object_bindings.get(name))
                    .and_then(|object_binding| {
                        object_binding_lookup_value(object_binding, &property)
                    })
                    .cloned();
                let current_enumerable = property_name.as_ref().is_some_and(|property_name| {
                    self.local_object_bindings
                        .get(name)
                        .or_else(|| self.module.global_object_bindings.get(name))
                        .map(|object_binding| {
                            !object_binding
                                .non_enumerable_string_properties
                                .iter()
                                .any(|hidden_name| hidden_name == property_name)
                        })
                        .unwrap_or(false)
                });
                let enumerable = descriptor.enumerable.unwrap_or(current_enumerable);
                let value = if descriptor.is_accessor() {
                    Expression::Undefined
                } else {
                    descriptor
                        .value
                        .as_ref()
                        .map(|expression| self.materialize_static_expression(expression))
                        .or_else(|| {
                            existing_value
                                .as_ref()
                                .map(|expression| self.materialize_static_expression(expression))
                        })
                        .unwrap_or(Expression::Undefined)
                };
                if let Some(object_binding) = self.local_object_bindings.get_mut(name) {
                    object_binding_define_property(
                        object_binding,
                        property.clone(),
                        value.clone(),
                        enumerable,
                    );
                } else if self.binding_name_is_global(name) {
                    let object_binding = self
                        .module
                        .global_object_bindings
                        .entry(name.to_string())
                        .or_insert_with(|| ObjectValueBinding {
                            string_properties: Vec::new(),
                            symbol_properties: Vec::new(),
                            non_enumerable_string_properties: Vec::new(),
                        });
                    object_binding_define_property(object_binding, property, value, enumerable);
                }
            }
            Expression::Member {
                object,
                property: target_property,
            } if matches!(target_property.as_ref(), Expression::String(name) if name == "prototype") =>
            {
                let Expression::Identifier(name) = object.as_ref() else {
                    return;
                };
                let property = self
                    .resolve_property_key_expression(property)
                    .unwrap_or_else(|| self.materialize_static_expression(property));
                let property_name = static_property_name_from_expression(&property);
                let existing_value = self
                    .local_prototype_object_bindings
                    .get(name)
                    .or_else(|| self.module.global_prototype_object_bindings.get(name))
                    .and_then(|object_binding| {
                        object_binding_lookup_value(object_binding, &property)
                    })
                    .cloned();
                let current_enumerable = property_name.as_ref().is_some_and(|property_name| {
                    self.local_prototype_object_bindings
                        .get(name)
                        .or_else(|| self.module.global_prototype_object_bindings.get(name))
                        .map(|object_binding| {
                            !object_binding
                                .non_enumerable_string_properties
                                .iter()
                                .any(|hidden_name| hidden_name == property_name)
                        })
                        .unwrap_or(false)
                });
                let enumerable = descriptor.enumerable.unwrap_or(current_enumerable);
                let value = if descriptor.is_accessor() {
                    Expression::Undefined
                } else {
                    descriptor
                        .value
                        .as_ref()
                        .map(|expression| self.materialize_static_expression(expression))
                        .or_else(|| {
                            existing_value
                                .as_ref()
                                .map(|expression| self.materialize_static_expression(expression))
                        })
                        .unwrap_or(Expression::Undefined)
                };
                if let Some(object_binding) = self.local_prototype_object_bindings.get_mut(name) {
                    object_binding_define_property(
                        object_binding,
                        property.clone(),
                        value.clone(),
                        enumerable,
                    );
                }
                if self.binding_name_is_global(name) {
                    let object_binding = self
                        .module
                        .global_prototype_object_bindings
                        .entry(name.to_string())
                        .or_insert_with(|| ObjectValueBinding {
                            string_properties: Vec::new(),
                            symbol_properties: Vec::new(),
                            non_enumerable_string_properties: Vec::new(),
                        });
                    object_binding_define_property(object_binding, property, value, enumerable);
                }
            }
            _ => {}
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_call_result_expression(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Option<Expression> {
        self.resolve_static_call_result_expression_with_context(
            callee,
            arguments,
            self.current_user_function_name.as_deref(),
        )
        .map(|(value, _)| value)
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_call_result_expression_with_context(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
        current_function_name: Option<&str>,
    ) -> Option<(Expression, Option<String>)> {
        if let Some(specialized) = self.resolve_specialized_function_value_from_expression(callee) {
            let LocalFunctionBinding::User(function_name) = &specialized.binding else {
                return None;
            };
            let user_function = self.module.user_function_map.get(function_name)?;
            let return_value = specialized.summary.return_value.as_ref()?;
            return Some((
                self.substitute_user_function_argument_bindings(
                    return_value,
                    user_function,
                    arguments,
                ),
                Some(function_name.clone()),
            ));
        }

        if let Expression::Member { object, property } = callee
            && let Some(specialized) =
                self.resolve_tracked_array_specialized_function_value(object, property)
        {
            let LocalFunctionBinding::User(function_name) = &specialized.binding else {
                return None;
            };
            let user_function = self.module.user_function_map.get(function_name)?;
            let return_value = specialized.summary.return_value.as_ref()?;
            return Some((
                self.substitute_user_function_argument_bindings(
                    return_value,
                    user_function,
                    arguments,
                ),
                Some(function_name.clone()),
            ));
        }

        if let Expression::Member { object, property } = callee
            && matches!(property.as_ref(), Expression::String(name) if name == "replace")
            && let [
                CallArgument::Expression(search_expression),
                CallArgument::Expression(replacement_expression),
            ] = arguments
            && let Some(text) = self.resolve_static_string_replace_result_with_context(
                object,
                search_expression,
                replacement_expression,
                current_function_name,
            )
        {
            return Some((Expression::String(text), None));
        }

        if let Expression::Member { object, property } = callee
            && matches!(object.as_ref(), Expression::Identifier(name) if name == "Object")
            && matches!(property.as_ref(), Expression::String(name) if name == "getPrototypeOf")
            && let [CallArgument::Expression(target), ..] = arguments
            && let Some(prototype) = self.resolve_static_object_prototype_expression(target)
        {
            return Some((prototype, None));
        }

        if let Expression::Member { object, property } = callee
            && matches!(object.as_ref(), Expression::Identifier(name) if name == "Object")
            && matches!(property.as_ref(), Expression::String(name) if name == "isExtensible")
            && let [CallArgument::Expression(target), ..] = arguments
        {
            return Some((
                Expression::Bool(
                    self.resolve_static_object_prototype_expression(target)
                        .is_some(),
                ),
                None,
            ));
        }

        if let Expression::Member { object, property } = callee
            && matches!(property.as_ref(), Expression::String(name) if name == "call" || name == "apply")
        {
            let function_binding = self.resolve_function_binding_from_expression_with_context(
                object,
                current_function_name,
            )?;
            let LocalFunctionBinding::User(function_name) = &function_binding else {
                return None;
            };
            let user_function = self.module.user_function_map.get(function_name)?;
            if !(user_function
                .inline_summary
                .as_ref()
                .is_some_and(|summary| summary.effects.is_empty())
                || self
                    .user_function_has_explicit_call_frame_inlineable_terminal_body(user_function))
            {
                return None;
            }

            let expanded_arguments = self.expand_call_arguments(arguments);
            let raw_this_expression = expanded_arguments
                .first()
                .cloned()
                .unwrap_or(Expression::Undefined);
            let call_arguments =
                if matches!(property.as_ref(), Expression::String(name) if name == "call") {
                    expanded_arguments.into_iter().skip(1).collect::<Vec<_>>()
                } else {
                    let apply_expression = expanded_arguments
                        .get(1)
                        .cloned()
                        .unwrap_or(Expression::Undefined);
                    self.expand_apply_call_arguments_from_expression(&apply_expression)?
                        .into_iter()
                        .map(|argument| match argument {
                            CallArgument::Expression(expression)
                            | CallArgument::Spread(expression) => expression,
                        })
                        .collect::<Vec<_>>()
                };
            let this_binding =
                if self.should_box_sloppy_function_this(user_function, &raw_this_expression) {
                    Expression::This
                } else {
                    self.materialize_static_expression(&raw_this_expression)
                };
            let value = self.resolve_function_binding_static_return_expression_with_call_frame(
                &function_binding,
                &call_arguments,
                &this_binding,
            )?;
            return Some((value, Some(function_name.clone())));
        }

        if let Expression::Call {
            callee: bind_callee,
            arguments: bind_arguments,
        } = callee
            && let Expression::Member { object, property } = bind_callee.as_ref()
            && matches!(property.as_ref(), Expression::String(name) if name == "bind")
        {
            let function_binding = self.resolve_function_binding_from_expression_with_context(
                object,
                current_function_name,
            )?;
            let LocalFunctionBinding::User(function_name) = &function_binding else {
                return None;
            };
            let user_function = self.module.user_function_map.get(function_name)?;
            if !(user_function
                .inline_summary
                .as_ref()
                .is_some_and(|summary| summary.effects.is_empty())
                || self
                    .user_function_has_explicit_call_frame_inlineable_terminal_body(user_function))
            {
                return None;
            }

            let expanded_bind_arguments = self.expand_call_arguments(bind_arguments);
            let raw_this_expression = expanded_bind_arguments
                .first()
                .cloned()
                .unwrap_or(Expression::Undefined);
            let call_arguments = expanded_bind_arguments
                .into_iter()
                .skip(1)
                .chain(self.expand_call_arguments(arguments))
                .collect::<Vec<_>>();
            let this_binding =
                if self.should_box_sloppy_function_this(user_function, &raw_this_expression) {
                    Expression::This
                } else {
                    self.materialize_static_expression(&raw_this_expression)
                };
            let value = self.resolve_function_binding_static_return_expression_with_call_frame(
                &function_binding,
                &call_arguments,
                &this_binding,
            )?;
            return Some((value, Some(function_name.clone())));
        }

        let binding = self
            .resolve_function_binding_from_expression_with_context(callee, current_function_name)?;
        if let Some(outcome) = self.resolve_static_function_outcome_from_binding_with_context(
            &binding,
            arguments,
            current_function_name,
        ) {
            return match outcome {
                StaticEvalOutcome::Value(value) => Some((
                    value,
                    match binding {
                        LocalFunctionBinding::User(function_name) => Some(function_name),
                        LocalFunctionBinding::Builtin(_) => None,
                    },
                )),
                StaticEvalOutcome::Throw(_) => None,
            };
        }

        let LocalFunctionBinding::User(function_name) = binding else {
            return None;
        };
        let user_function = self.module.user_function_map.get(&function_name)?;

        let summary = user_function.inline_summary.as_ref()?;
        if !summary.effects.is_empty() {
            return None;
        }
        let return_value = summary.return_value.as_ref()?;
        Some((
            self.substitute_user_function_argument_bindings(return_value, user_function, arguments),
            Some(function_name),
        ))
    }

    pub(in crate::backend::direct_wasm) fn update_local_arguments_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        if self.is_direct_arguments_object(value) {
            self.direct_arguments_aliases.insert(name.to_string());
            self.local_arguments_bindings.remove(name);
            self.local_kinds
                .insert(name.to_string(), StaticValueKind::Object);
            return;
        }
        self.direct_arguments_aliases.remove(name);
        let Some(arguments_binding) = self.resolve_arguments_binding_from_expression(value) else {
            self.local_arguments_bindings.remove(name);
            return;
        };
        self.local_arguments_bindings
            .insert(name.to_string(), arguments_binding);
        self.local_kinds
            .insert(name.to_string(), StaticValueKind::Object);
    }
}
