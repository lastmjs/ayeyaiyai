use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn materialize_member_expression(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Expression {
        let resolved_object = self
            .resolve_bound_alias_expression(object)
            .filter(|resolved| !static_expression_matches(resolved, object));
        let resolved_property = self.resolve_property_key_expression(property).or_else(|| {
            self.resolve_bound_alias_expression(property)
                .filter(|resolved| !static_expression_matches(resolved, property))
        });
        if matches!(property, Expression::String(name) if name == "prototype") {
            let materialized_object = self.materialize_static_expression(object);
            if matches!(
                materialized_object,
                Expression::Identifier(_) | Expression::New { .. }
            ) {
                return Expression::Member {
                    object: Box::new(materialized_object),
                    property: Box::new(Expression::String("prototype".to_string())),
                };
            }
        }
        if let Some(step_binding) = self.resolve_iterator_step_binding_from_expression(object)
            && let Expression::String(property_name) = property
        {
            match (property_name.as_str(), step_binding) {
                (
                    "done",
                    IteratorStepBinding::Runtime {
                        static_done: Some(done),
                        ..
                    },
                ) => return Expression::Bool(done),
                (
                    "value",
                    IteratorStepBinding::Runtime {
                        static_value: Some(value),
                        ..
                    },
                ) => return self.materialize_static_expression(&value),
                _ => {}
            }
        }
        if self.expression_uses_runtime_dynamic_binding(object) {
            return Expression::Member {
                object: Box::new(self.materialize_static_expression(object)),
                property: Box::new(self.materialize_static_expression(property)),
            };
        }
        let materialized_object = self.materialize_static_expression(object);
        let materialized_property = self.materialize_static_expression(property);
        if matches!(materialized_object, Expression::This)
            && let Expression::String(property_name) = &materialized_property
            && let Some(descriptor) =
                self.resolve_top_level_global_property_descriptor_binding(property_name)
            && let Some(value) = descriptor.value
        {
            return self.materialize_static_expression(&value);
        }
        if let Some(function_name) = self
            .resolve_function_name_value(object, property)
            .or_else(|| {
                resolved_object.as_ref().and_then(|resolved_object| {
                    self.resolve_function_name_value(resolved_object, property)
                })
            })
            .or_else(|| {
                resolved_property.as_ref().and_then(|resolved_property| {
                    self.resolve_function_name_value(object, resolved_property)
                })
            })
            .or_else(|| {
                resolved_object.as_ref().and_then(|resolved_object| {
                    resolved_property.as_ref().and_then(|resolved_property| {
                        self.resolve_function_name_value(resolved_object, resolved_property)
                    })
                })
            })
            .or_else(|| {
                (!static_expression_matches(&materialized_object, object)
                    || !static_expression_matches(&materialized_property, property))
                .then(|| {
                    self.resolve_function_name_value(&materialized_object, &materialized_property)
                })?
            })
        {
            return Expression::String(function_name);
        }
        if let Some(getter_binding) = self
            .resolve_member_getter_binding_shallow(object, &materialized_property)
            .or_else(|| {
                resolved_object.as_ref().and_then(|resolved_object| {
                    self.resolve_member_getter_binding_shallow(
                        resolved_object,
                        &materialized_property,
                    )
                })
            })
            .or_else(|| {
                resolved_property.as_ref().and_then(|resolved_property| {
                    self.resolve_member_getter_binding_shallow(object, resolved_property)
                })
            })
            .or_else(|| {
                resolved_object.as_ref().and_then(|resolved_object| {
                    resolved_property.as_ref().and_then(|resolved_property| {
                        self.resolve_member_getter_binding_shallow(
                            resolved_object,
                            resolved_property,
                        )
                    })
                })
            })
            .or_else(|| {
                self.resolve_member_getter_binding_shallow(
                    &materialized_object,
                    &materialized_property,
                )
            })
        {
            if let Some(value) = self.resolve_static_getter_value_from_binding_with_context(
                &getter_binding,
                object,
                self.current_function_name(),
            ) {
                return self.materialize_static_expression(&value);
            }
            return Expression::Member {
                object: Box::new(materialized_object),
                property: Box::new(materialized_property),
            };
        }
        if let Some(array_binding) = self.resolve_array_binding_from_expression(object) {
            if matches!(property, Expression::String(text) if text == "length") {
                if self
                    .runtime_array_length_local_for_expression(object)
                    .is_some()
                {
                    return Expression::Member {
                        object: Box::new(self.materialize_static_expression(object)),
                        property: Box::new(self.materialize_static_expression(property)),
                    };
                }
                return Expression::Number(array_binding.values.len() as f64);
            }
            if let Some(index) = argument_index_from_expression(property) {
                if let Some(Some(value)) = array_binding.values.get(index as usize) {
                    return self.materialize_static_expression(value);
                }
                let has_runtime_array_state = self
                    .runtime_array_length_local_for_expression(object)
                    .is_some()
                    || matches!(
                        object,
                        Expression::Identifier(name)
                            if self.is_named_global_array_binding(name)
                                && self.uses_global_runtime_array_state(name)
                    );
                if has_runtime_array_state {
                    return Expression::Member {
                        object: Box::new(self.materialize_static_expression(object)),
                        property: Box::new(self.materialize_static_expression(property)),
                    };
                }
                return Expression::Undefined;
            }
        }
        if let Some(object_binding) = self.resolve_object_binding_from_expression(object) {
            if let Some(getter_binding) = self
                .resolve_member_getter_binding(object, &materialized_property)
                .or_else(|| {
                    resolved_object.as_ref().and_then(|resolved_object| {
                        self.resolve_member_getter_binding(resolved_object, &materialized_property)
                    })
                })
                .or_else(|| {
                    resolved_property.as_ref().and_then(|resolved_property| {
                        self.resolve_member_getter_binding(object, resolved_property)
                    })
                })
                .or_else(|| {
                    resolved_object.as_ref().and_then(|resolved_object| {
                        resolved_property.as_ref().and_then(|resolved_property| {
                            self.resolve_member_getter_binding(resolved_object, resolved_property)
                        })
                    })
                })
                .or_else(|| {
                    self.resolve_member_getter_binding(&materialized_object, &materialized_property)
                })
            {
                if let Some(value) = self.resolve_static_getter_value_from_binding_with_context(
                    &getter_binding,
                    object,
                    self.current_function_name(),
                ) {
                    return self.materialize_static_expression(&value);
                }
                return Expression::Member {
                    object: Box::new(materialized_object),
                    property: Box::new(materialized_property),
                };
            }
            if let Some(value) =
                object_binding_lookup_value(&object_binding, &materialized_property)
            {
                return self.materialize_static_expression(value);
            }
            if self
                .resolve_member_function_binding(object, &materialized_property)
                .or_else(|| {
                    self.resolve_member_function_binding(
                        &materialized_object,
                        &materialized_property,
                    )
                })
                .is_some()
            {
                return Expression::Member {
                    object: Box::new(materialized_object),
                    property: Box::new(materialized_property),
                };
            }
            if static_property_name_from_expression(&materialized_property).is_some()
                || object_binding_has_property(&object_binding, &materialized_property)
            {
                return Expression::Undefined;
            }
        }
        if let Expression::String(text) = object {
            if let Some(index) = argument_index_from_expression(property) {
                return text
                    .chars()
                    .nth(index as usize)
                    .map(|character| Expression::String(character.to_string()))
                    .unwrap_or(Expression::Undefined);
            }
        }
        Expression::Member {
            object: Box::new(materialized_object),
            property: Box::new(materialized_property),
        }
    }
}
