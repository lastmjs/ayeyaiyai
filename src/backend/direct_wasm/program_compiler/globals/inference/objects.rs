use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn infer_global_object_binding(
        &self,
        expression: &Expression,
    ) -> Option<ObjectValueBinding> {
        let mut value_bindings = self.global_value_bindings.clone();
        let mut object_bindings = self.global_object_bindings.clone();
        self.infer_global_object_binding_with_state(
            expression,
            &mut value_bindings,
            &mut object_bindings,
        )
    }

    pub(in crate::backend::direct_wasm) fn infer_global_object_binding_with_state(
        &self,
        expression: &Expression,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<ObjectValueBinding> {
        match expression {
            Expression::Identifier(name) => object_bindings.get(name).cloned().or_else(|| {
                value_bindings
                    .get(name)
                    .cloned()
                    .filter(
                        |value| !matches!(value, Expression::Identifier(alias) if alias == name),
                    )
                    .and_then(|value| {
                        self.infer_global_object_binding_with_state(
                            &value,
                            value_bindings,
                            object_bindings,
                        )
                    })
            }),
            Expression::Object(entries) => {
                let mut object_binding = empty_object_value_binding();
                for entry in entries {
                    match entry {
                        crate::ir::hir::ObjectEntry::Data { key, value } => {
                            let materialized_key = self
                                .materialize_global_expression_with_state(
                                    key,
                                    &HashMap::new(),
                                    value_bindings,
                                    object_bindings,
                                )
                                .unwrap_or_else(|| self.materialize_global_expression(key));
                            let value = self
                                .materialize_global_expression_with_state(
                                    value,
                                    &HashMap::new(),
                                    value_bindings,
                                    object_bindings,
                                )
                                .unwrap_or_else(|| self.materialize_global_expression(value));
                            object_binding_set_property(
                                &mut object_binding,
                                materialized_key,
                                value,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, .. }
                        | crate::ir::hir::ObjectEntry::Setter { key, .. } => {
                            let materialized_key = self
                                .materialize_global_expression_with_state(
                                    key,
                                    &HashMap::new(),
                                    value_bindings,
                                    object_bindings,
                                )
                                .unwrap_or_else(|| self.materialize_global_expression(key));
                            object_binding_set_property(
                                &mut object_binding,
                                materialized_key,
                                Expression::Undefined,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Spread(expression) => {
                            let spread_expression = self
                                .materialize_global_expression_with_state(
                                    expression,
                                    &HashMap::new(),
                                    value_bindings,
                                    object_bindings,
                                )
                                .unwrap_or_else(|| self.materialize_global_expression(expression));
                            if matches!(spread_expression, Expression::Null | Expression::Undefined)
                                || matches!(
                                    &spread_expression,
                                    Expression::Identifier(name)
                                        if name == "undefined"
                                            && !self.global_bindings.contains_key(name)
                                            && !self.global_lexical_bindings.contains(name)
                                )
                            {
                                continue;
                            }
                            let spread_binding = self
                                .infer_global_copy_data_properties_binding_with_state(
                                    &spread_expression,
                                    value_bindings,
                                    object_bindings,
                                )?;
                            merge_enumerable_object_binding(&mut object_binding, &spread_binding);
                        }
                    }
                }
                Some(object_binding)
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn infer_global_copy_data_properties_binding_with_state(
        &self,
        expression: &Expression,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<ObjectValueBinding> {
        let source_binding = self.infer_global_object_binding_with_state(
            expression,
            value_bindings,
            object_bindings,
        )?;
        let mut copied_binding = empty_object_value_binding();
        for (name, _) in &source_binding.string_properties {
            if source_binding
                .non_enumerable_string_properties
                .iter()
                .any(|hidden_name| hidden_name == name)
            {
                continue;
            }
            let property = Expression::String(name.clone());
            let copied_value = self
                .infer_global_member_getter_return_value_with_state(
                    expression,
                    &property,
                    value_bindings,
                    object_bindings,
                )
                .or_else(|| {
                    self.infer_global_object_binding_with_state(
                        expression,
                        value_bindings,
                        object_bindings,
                    )
                    .and_then(|binding| object_binding_lookup_value(&binding, &property).cloned())
                })
                .unwrap_or(Expression::Undefined);
            object_binding_set_property(&mut copied_binding, property, copied_value);
        }
        for (property, _) in &source_binding.symbol_properties {
            let copied_value = self
                .infer_global_member_getter_return_value_with_state(
                    expression,
                    property,
                    value_bindings,
                    object_bindings,
                )
                .or_else(|| {
                    self.infer_global_object_binding_with_state(
                        expression,
                        value_bindings,
                        object_bindings,
                    )
                    .and_then(|binding| object_binding_lookup_value(&binding, property).cloned())
                })
                .unwrap_or(Expression::Undefined);
            object_binding_set_property(&mut copied_binding, property.clone(), copied_value);
        }
        Some(copied_binding)
    }

    #[cfg(test)]

    pub(in crate::backend::direct_wasm) fn infer_global_copy_data_properties_binding(
        &self,
        expression: &Expression,
    ) -> Option<ObjectValueBinding> {
        let mut value_bindings = self.global_value_bindings.clone();
        let mut object_bindings = self.global_object_bindings.clone();
        self.infer_global_copy_data_properties_binding_with_state(
            expression,
            &mut value_bindings,
            &mut object_bindings,
        )
    }

    pub(in crate::backend::direct_wasm) fn infer_global_member_getter_return_value_with_state(
        &self,
        object: &Expression,
        property: &Expression,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<Expression> {
        let getter_binding = self.infer_global_member_getter_binding(object, property)?;
        self.execute_global_function_binding_with_state(
            &getter_binding,
            &[],
            value_bindings,
            object_bindings,
        )
    }

    #[cfg(test)]

    pub(in crate::backend::direct_wasm) fn infer_global_member_getter_return_value(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<Expression> {
        let mut value_bindings = self.global_value_bindings.clone();
        let mut object_bindings = self.global_object_bindings.clone();
        self.infer_global_member_getter_return_value_with_state(
            object,
            property,
            &mut value_bindings,
            &mut object_bindings,
        )
    }

    pub(in crate::backend::direct_wasm) fn infer_global_member_getter_binding(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<LocalFunctionBinding> {
        let target = match object {
            Expression::Identifier(name) => MemberFunctionBindingTarget::Identifier(name.clone()),
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "prototype") =>
            {
                let Expression::Identifier(name) = object.as_ref() else {
                    return None;
                };
                MemberFunctionBindingTarget::Prototype(name.clone())
            }
            Expression::New { callee, .. } => {
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                MemberFunctionBindingTarget::Prototype(name.clone())
            }
            _ => return None,
        };
        let property = if let Some(property_name) = static_property_name_from_expression(property) {
            MemberFunctionBindingProperty::String(property_name)
        } else {
            return None;
        };
        let key = MemberFunctionBindingKey { target, property };
        self.global_member_getter_bindings.get(&key).cloned()
    }

    pub(in crate::backend::direct_wasm) fn infer_global_member_function_binding_property(
        &self,
        property: &Expression,
    ) -> Option<MemberFunctionBindingProperty> {
        let resolved_property = self.materialize_global_expression(property);
        static_property_name_from_expression(&resolved_property)
            .map(MemberFunctionBindingProperty::String)
    }
}
