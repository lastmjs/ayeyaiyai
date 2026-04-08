use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn materialize_global_expression_with_state(
        &self,
        expression: &Expression,
        local_bindings: &HashMap<String, Expression>,
        value_bindings: &HashMap<String, Expression>,
        object_bindings: &HashMap<String, ObjectValueBinding>,
    ) -> Option<Expression> {
        let context = self.static_eval_context();
        materialize_expression_in_binding_maps(
            &context,
            expression,
            local_bindings,
            value_bindings,
            object_bindings,
            &|expression, local_bindings, value_bindings, object_bindings| {
                resolve_stateful_object_binding_in_binding_maps(
                    expression,
                    local_bindings,
                    value_bindings,
                    object_bindings,
                    &|expression, _local_bindings, value_bindings, object_bindings| {
                        self.infer_global_object_binding_with_state(
                            expression,
                            &mut value_bindings.clone(),
                            &mut object_bindings.clone(),
                        )
                    },
                )
            },
            &|object, property| {
                preserves_missing_member_function_capture(
                    object,
                    property,
                    |object, property| self.global_member_function_binding_key(object, property),
                    |key| self.has_global_member_function_capture_slots(key),
                )
            },
        )
    }

    pub(in crate::backend::direct_wasm) fn materialize_global_expression(
        &self,
        expression: &Expression,
    ) -> Expression {
        match expression {
            Expression::Identifier(name) => {
                if name == "undefined"
                    && !self.global_has_binding(name)
                    && !self.global_has_lexical_binding(name)
                {
                    return Expression::Undefined;
                }
                if self.global_binding_kind(name) == Some(StaticValueKind::Symbol) {
                    return expression.clone();
                }
                if let Some(value) = self.global_value_binding(name) {
                    if self.global_object_binding(name).is_some()
                        && matches!(value, Expression::Object(_) | Expression::Identifier(_))
                    {
                        return Expression::Identifier(name.clone());
                    }
                    if !matches!(value, Expression::Identifier(alias) if alias == name) {
                        return self.materialize_global_expression(value);
                    }
                }
                expression.clone()
            }
            Expression::Member { object, property } => {
                if self
                    .global_member_function_binding_key(object, property)
                    .is_some_and(|key| self.has_global_member_function_capture_slots(&key))
                {
                    return expression.clone();
                }
                if let Some(array_binding) = self.infer_global_array_binding(object)
                    && let Some(index) = argument_index_from_expression(property)
                {
                    if let Some(Some(value)) = array_binding.values.get(index as usize) {
                        return self.materialize_global_expression(value);
                    }
                    return Expression::Undefined;
                }
                if let Some(object_binding) = self.infer_global_object_binding(object) {
                    let materialized_property = self.materialize_global_expression(property);
                    if let Some(value) =
                        object_binding_lookup_value(&object_binding, &materialized_property)
                    {
                        return self.materialize_global_expression(value);
                    }
                    if static_property_name_from_expression(&materialized_property).is_some()
                        || object_binding_has_property(&object_binding, &materialized_property)
                    {
                        return Expression::Undefined;
                    }
                }
                if let Expression::String(text) = object.as_ref()
                    && let Some(index) = argument_index_from_expression(property)
                {
                    return text
                        .chars()
                        .nth(index as usize)
                        .map(|character| Expression::String(character.to_string()))
                        .unwrap_or(Expression::Undefined);
                }
                let materialized_property = self.materialize_global_expression(property);
                let materialized = Expression::Member {
                    object: Box::new(self.materialize_global_expression(object)),
                    property: Box::new(materialized_property.clone()),
                };
                materialize_missing_member_expression_with_policy(
                    expression,
                    object,
                    materialized_property,
                    &(),
                    &|expression, _| Some(self.materialize_global_expression(expression)),
                    &|_full_expression, object, property, _environment| {
                        preserves_missing_member_function_capture(
                            object,
                            property,
                            |object, property| {
                                self.global_member_function_binding_key(object, property)
                            },
                            |key| self.has_global_member_function_capture_slots(key),
                        )
                    },
                )
                .unwrap_or(materialized)
            }
            Expression::Call { callee, arguments } => {
                if let Some(value) = self.infer_static_call_result_expression(callee, arguments) {
                    return self.materialize_global_expression(&value);
                }
                materialize_recursive_expression(expression, true, true, &|expression| {
                    Some(self.materialize_global_expression(expression))
                })
                .expect("program-side recursive materialization supports generic call rebuild")
            }
            _ => materialize_recursive_expression(expression, true, true, &|expression| {
                Some(self.materialize_global_expression(expression))
            })
            .unwrap_or_else(|| expression.clone()),
        }
    }
}
