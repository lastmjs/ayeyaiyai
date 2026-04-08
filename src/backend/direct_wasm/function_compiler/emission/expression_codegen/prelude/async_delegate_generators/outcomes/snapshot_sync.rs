use super::*;

impl<'a> FunctionCompiler<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::backend::direct_wasm) fn sync_async_yield_delegate_snapshot_after_step_result(
        &mut self,
        plan: &AsyncYieldDelegateGeneratorPlan,
        snapshot_bindings: &mut HashMap<String, Expression>,
        property_name: &str,
        step_result_name: &str,
        promise_done_name: &str,
        promise_value_name: &str,
        delegate_completion_name: &str,
        delegate_iterator_name: &str,
        static_step_result_has_accessor_properties: bool,
    ) {
        if snapshot_bindings.contains_key(promise_done_name)
            && self.resolve_static_boolean_expression(
                snapshot_bindings
                    .get(promise_done_name)
                    .expect("checked above"),
            ) == Some(true)
        {
            let completion_value = snapshot_bindings
                .get(delegate_completion_name)
                .cloned()
                .unwrap_or(Expression::Identifier(delegate_completion_name.to_string()));
            snapshot_bindings
                .entry(delegate_completion_name.to_string())
                .or_insert_with(|| completion_value.clone());
            if property_name != "return" {
                self.execute_bound_snapshot_statements(
                    &plan.completion_effects,
                    snapshot_bindings,
                    Some(&plan.function_name),
                );
            }
            let promise_value = match property_name {
                "return" => completion_value,
                "next" | "throw" => self
                    .evaluate_bound_snapshot_expression(
                        &plan.completion_value,
                        snapshot_bindings,
                        self.current_function_name(),
                    )
                    .unwrap_or_else(|| plan.completion_value.clone()),
                _ => Expression::Undefined,
            };
            snapshot_bindings.insert(promise_value_name.to_string(), promise_value.clone());
            self.update_local_value_binding(promise_value_name, &promise_value);
        }

        if !static_step_result_has_accessor_properties
            && let Some(step_result_binding) = self.resolve_object_binding_from_expression(
                &Expression::Identifier(step_result_name.to_string()),
            )
            && let Some(done_value) = self.resolve_object_binding_property_value(
                &step_result_binding,
                &Expression::String("done".to_string()),
            )
        {
            self.update_local_value_binding(promise_done_name, &done_value);
            if let Some(done) = self.resolve_static_boolean_expression(&done_value)
                && !done
                && let Some(yield_value) = self.resolve_object_binding_property_value(
                    &step_result_binding,
                    &Expression::String("value".to_string()),
                )
            {
                self.update_local_value_binding(promise_value_name, &yield_value);
            }
        }

        if !snapshot_bindings.contains_key(promise_done_name) {
            let done_member = Expression::Member {
                object: Box::new(Expression::Identifier(step_result_name.to_string())),
                property: Box::new(Expression::String("done".to_string())),
            };
            if let Some(done_value) = self.evaluate_bound_snapshot_expression(
                &done_member,
                snapshot_bindings,
                self.current_function_name(),
            ) {
                snapshot_bindings.insert(promise_done_name.to_string(), done_value.clone());
                self.update_local_value_binding(promise_done_name, &done_value);
                if let Some(done) = self.resolve_static_boolean_expression(&done_value) {
                    if done {
                        let completion_value = self.evaluate_bound_snapshot_expression(
                            &Expression::Member {
                                object: Box::new(Expression::Identifier(
                                    step_result_name.to_string(),
                                )),
                                property: Box::new(Expression::String("value".to_string())),
                            },
                            snapshot_bindings,
                            self.current_function_name(),
                        );
                        if let Some(completion_value) = completion_value {
                            snapshot_bindings.insert(
                                delegate_completion_name.to_string(),
                                completion_value.clone(),
                            );
                            self.update_local_value_binding(
                                delegate_completion_name,
                                &completion_value,
                            );
                            if property_name != "return" {
                                self.execute_bound_snapshot_statements(
                                    &plan.completion_effects,
                                    snapshot_bindings,
                                    Some(&plan.function_name),
                                );
                            }
                        }
                        let promise_value = match property_name {
                            "return" => snapshot_bindings
                                .get(delegate_completion_name)
                                .cloned()
                                .unwrap_or(Expression::Identifier(
                                    delegate_completion_name.to_string(),
                                )),
                            "next" | "throw" => self
                                .evaluate_bound_snapshot_expression(
                                    &plan.completion_value,
                                    snapshot_bindings,
                                    self.current_function_name(),
                                )
                                .unwrap_or_else(|| plan.completion_value.clone()),
                            _ => Expression::Undefined,
                        };
                        snapshot_bindings
                            .insert(promise_value_name.to_string(), promise_value.clone());
                        self.update_local_value_binding(promise_value_name, &promise_value);
                    } else if let Some(yield_value) = self.evaluate_bound_snapshot_expression(
                        &Expression::Member {
                            object: Box::new(Expression::Identifier(step_result_name.to_string())),
                            property: Box::new(Expression::String("value".to_string())),
                        },
                        snapshot_bindings,
                        self.current_function_name(),
                    ) {
                        if !self.async_yield_delegate_step_value_is_placeholder(
                            &yield_value,
                            delegate_iterator_name,
                            property_name,
                        ) {
                            snapshot_bindings
                                .insert(promise_value_name.to_string(), yield_value.clone());
                            self.update_local_value_binding(promise_value_name, &yield_value);
                        }
                    }
                }
            }
        }
    }

    pub(super) fn async_yield_delegate_step_value_is_placeholder(
        &self,
        expression: &Expression,
        delegate_iterator_name: &str,
        property_name: &str,
    ) -> bool {
        matches!(
            expression,
            Expression::Member { object, property }
                if matches!(property.as_ref(), Expression::String(name) if name == "value")
                    && matches!(
                        object.as_ref(),
                        Expression::Call { callee, .. }
                            if matches!(
                                callee.as_ref(),
                                Expression::Member { object, property }
                                    if matches!(object.as_ref(), Expression::Identifier(name) if name == delegate_iterator_name)
                                        && matches!(property.as_ref(), Expression::String(method_name) if method_name == property_name)
                            )
                    )
        )
    }
}
