use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_array_slice_binding(
        &self,
        object: &Expression,
        arguments: &[CallArgument],
    ) -> Option<ArrayValueBinding> {
        let array_binding = self.resolve_array_binding_from_expression(object)?;
        let start = match arguments.first() {
            None => 0usize,
            Some(CallArgument::Expression(expression)) | Some(CallArgument::Spread(expression)) => {
                self.resolve_static_number_value(expression)?.max(0.0) as usize
            }
        };
        let end = match arguments.get(1) {
            None => array_binding.values.len(),
            Some(CallArgument::Expression(expression)) | Some(CallArgument::Spread(expression)) => {
                self.resolve_static_number_value(expression)?.max(0.0) as usize
            }
        };
        let start = start.min(array_binding.values.len());
        let end = end.min(array_binding.values.len()).max(start);
        Some(ArrayValueBinding {
            values: array_binding.values[start..end].to_vec(),
        })
    }

    pub(in crate::backend::direct_wasm) fn resolve_array_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<ArrayValueBinding> {
        if let Expression::Identifier(name) = expression {
            if let Some(binding) = self
                .state
                .speculation
                .static_semantics
                .local_typed_array_view_binding(name)
                .and_then(|view| self.typed_array_view_static_values(view))
                .or_else(|| {
                    self.state
                        .speculation
                        .static_semantics
                        .local_array_binding(name)
                        .cloned()
                })
                .or_else(|| {
                    let hidden_name = self.resolve_user_function_capture_hidden_name(name)?;
                    self.backend
                        .global_semantics
                        .values
                        .array_bindings
                        .get(&hidden_name)
                        .cloned()
                })
                .or_else(|| {
                    self.backend
                        .global_semantics
                        .values
                        .array_bindings
                        .get(name)
                        .cloned()
                })
            {
                return Some(binding);
            }
        }

        if let Some(resolved) = self
            .resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
        {
            return self.resolve_array_binding_from_expression(&resolved);
        }

        let binding = match expression {
            Expression::Assign { value, .. } | Expression::AssignSuperMember { value, .. } => {
                self.resolve_array_binding_from_expression(value)
            }
            Expression::Sequence(expressions) => expressions
                .last()
                .and_then(|expression| self.resolve_array_binding_from_expression(expression)),
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                let branch = if self.resolve_static_if_condition_value(condition)? {
                    then_expression.as_ref()
                } else {
                    else_expression.as_ref()
                };
                self.resolve_array_binding_from_expression(branch)
            }
            Expression::Identifier(name) => self
                .state
                .speculation
                .static_semantics
                .local_typed_array_view_binding(name)
                .and_then(|view| self.typed_array_view_static_values(view))
                .or_else(|| {
                    self.state
                        .speculation
                        .static_semantics
                        .local_array_binding(name)
                        .cloned()
                })
                .or_else(|| {
                    let hidden_name = self.resolve_user_function_capture_hidden_name(name)?;
                    self.backend
                        .global_semantics
                        .values
                        .array_bindings
                        .get(&hidden_name)
                        .cloned()
                })
                .or_else(|| {
                    self.backend
                        .global_semantics
                        .values
                        .array_bindings
                        .get(name)
                        .cloned()
                }),
            Expression::EnumerateKeys(value) => self.static_enumerated_keys_binding(value),
            Expression::Member { object, property } => {
                let property = self
                    .resolve_property_key_expression(property)
                    .unwrap_or_else(|| self.materialize_static_expression(property));
                if let Some(object_binding) = self.resolve_object_binding_from_expression(object)
                    && let Some(value) =
                        self.resolve_object_binding_property_value(&object_binding, &property)
                    && let Some(array_binding) = self.resolve_array_binding_from_expression(&value)
                {
                    return Some(array_binding);
                }
                let array_binding = self.resolve_array_binding_from_expression(object)?;
                let index = argument_index_from_expression(&property)?;
                let value = array_binding.values.get(index as usize)?.clone()?;
                self.resolve_array_binding_from_expression(&value)
            }
            Expression::Call { callee, arguments } => {
                if let Some(binding) =
                    self.static_builtin_object_array_call_binding(callee, arguments)
                {
                    return Some(binding);
                }
                if let Expression::Member { object, property } = callee.as_ref() {
                    if matches!(property.as_ref(), Expression::String(name) if name == "slice") {
                        return self.resolve_array_slice_binding(object, arguments);
                    }
                }
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                let user_function = self.resolve_user_function_from_callee_name(name)?;
                let param_index = user_function.enumerated_keys_param_index?;
                let argument = match arguments.get(param_index) {
                    Some(CallArgument::Expression(expression))
                    | Some(CallArgument::Spread(expression)) => expression,
                    None => return Some(ArrayValueBinding { values: Vec::new() }),
                };
                self.static_enumerated_keys_binding(argument)
            }
            Expression::New { callee, arguments } => {
                if matches!(callee.as_ref(), Expression::Identifier(name) if name == "Array" && self.is_unshadowed_builtin_identifier(name))
                {
                    let expanded_arguments = self.expand_call_arguments(arguments);
                    if expanded_arguments.is_empty() {
                        return Some(ArrayValueBinding { values: Vec::new() });
                    }
                    if expanded_arguments.len() == 1
                        && let Some(length) =
                            self.resolve_static_number_value(&expanded_arguments[0])
                        && length.is_finite()
                        && length >= 0.0
                        && length.fract() == 0.0
                    {
                        return Some(ArrayValueBinding {
                            values: vec![None; length as usize],
                        });
                    }
                    return Some(ArrayValueBinding {
                        values: expanded_arguments.into_iter().map(Some).collect(),
                    });
                }
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                let user_function = self.resolve_user_function_from_callee_name(name)?;
                let param_index = user_function.enumerated_keys_param_index?;
                let argument = match arguments.get(param_index) {
                    Some(CallArgument::Expression(expression))
                    | Some(CallArgument::Spread(expression)) => expression,
                    None => return Some(ArrayValueBinding { values: Vec::new() }),
                };
                self.static_enumerated_keys_binding(argument)
            }
            Expression::Array(elements) => {
                let mut values = Vec::new();
                for element in elements {
                    match element {
                        crate::ir::hir::ArrayElement::Expression(expression) => {
                            values.push(Some(self.materialize_static_expression(expression)));
                        }
                        crate::ir::hir::ArrayElement::Spread(expression) => {
                            if let Some(binding) =
                                self.resolve_array_binding_from_expression(expression)
                            {
                                values.extend(binding.values);
                            } else if let Some(binding) =
                                self.resolve_static_iterable_binding_from_expression(expression)
                            {
                                values.extend(binding.values);
                            } else {
                                values.push(Some(self.materialize_static_expression(expression)));
                            }
                        }
                    }
                }
                Some(ArrayValueBinding { values })
            }
            _ => None,
        };
        if binding.is_some() {
            return binding;
        }

        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.resolve_array_binding_from_expression(&materialized);
        }
        None
    }
}
