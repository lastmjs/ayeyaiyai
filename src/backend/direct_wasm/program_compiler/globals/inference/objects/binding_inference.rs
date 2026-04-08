use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn infer_global_object_binding(
        &self,
        expression: &Expression,
    ) -> Option<ObjectValueBinding> {
        self.with_cloned_global_binding_state(|value_bindings, object_bindings| {
            self.infer_global_object_binding_with_state(expression, value_bindings, object_bindings)
        })
    }

    pub(in crate::backend::direct_wasm) fn infer_global_object_binding_with_state(
        &self,
        expression: &Expression,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<ObjectValueBinding> {
        match expression {
            Expression::Identifier(name) => object_bindings
                .get(name)
                .cloned()
                .or_else(|| self.global_prototype_object_binding(name).cloned())
                .or_else(|| {
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
            Expression::Member { object, property }
                if matches!(property.as_ref(), Expression::String(name) if name == "prototype") =>
            {
                let Expression::Identifier(name) = object.as_ref() else {
                    return None;
                };
                self.global_prototype_object_binding(name).cloned()
            }
            _ => resolve_specialized_object_binding_expression(
                expression,
                &mut (value_bindings, object_bindings),
                |expression, _| self.infer_global_array_binding(expression),
                |entries, (value_bindings, object_bindings)| {
                    let context = self.static_eval_context();
                    resolve_structural_object_binding_in_environment(
                        &context,
                        entries,
                        &mut (value_bindings, object_bindings),
                        &|expression, (value_bindings, object_bindings)| {
                            let local_bindings = HashMap::new();
                            Some(
                                self.materialize_global_expression_with_state(
                                    expression,
                                    &local_bindings,
                                    value_bindings,
                                    object_bindings,
                                )
                                .unwrap_or_else(|| self.materialize_global_expression(expression)),
                            )
                        },
                        &|expression, (value_bindings, object_bindings)| {
                            self.infer_global_object_binding_with_state(
                                expression,
                                value_bindings,
                                object_bindings,
                            )
                        },
                        &|object, property, (value_bindings, object_bindings)| {
                            self.infer_global_member_getter_return_value_with_state(
                                object,
                                property,
                                value_bindings,
                                object_bindings,
                            )
                        },
                    )
                },
                |expression, _| {
                    matches!(
                        expression,
                        Expression::Call { callee, .. }
                            if matches!(
                                callee.as_ref(),
                                Expression::Member { object, property }
                                    if matches!(object.as_ref(), Expression::Identifier(name) if name == "Object")
                                        && matches!(property.as_ref(), Expression::String(name) if name == "create")
                            )
                    )
                },
                |_, _| None,
            ),
        }
    }
}
