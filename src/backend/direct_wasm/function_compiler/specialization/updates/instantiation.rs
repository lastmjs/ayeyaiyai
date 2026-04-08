use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn instantiate_specialized_function_value(
        &mut self,
        template: &SpecializedFunctionValue,
    ) -> DirectResult<Option<SpecializedFunctionValue>> {
        let captured = self.collect_capture_bindings_from_summary(&template.summary);
        if captured.is_empty() {
            return Ok(None);
        }

        let mut bindings = HashMap::new();
        for name in captured {
            if let Some((resolved_name, _)) = self.resolve_current_local_binding(&name) {
                let hidden_name = self.allocate_named_hidden_local(
                    "capture",
                    self.lookup_identifier_kind(&resolved_name)
                        .unwrap_or(StaticValueKind::Unknown),
                );
                self.emit_numeric_expression(&Expression::Identifier(resolved_name.clone()))?;
                let hidden_local = self
                    .state
                    .runtime
                    .locals
                    .get(&hidden_name)
                    .copied()
                    .expect("hidden capture local should be allocated");
                self.push_local_set(hidden_local);
                self.alias_runtime_binding_metadata(&hidden_name, &resolved_name);
                bindings.insert(name, Expression::Identifier(hidden_name));
                continue;
            }

            let Some(scope_object) = self.resolve_with_scope_binding_for_specialization(&name)
            else {
                continue;
            };
            if let Expression::Identifier(scope_name) = &scope_object
                && self
                    .parameter_scope_arguments_local_for(scope_name)
                    .is_none()
                && self.resolve_current_local_binding(scope_name).is_none()
                && self
                    .resolve_eval_local_function_hidden_name(scope_name)
                    .is_none()
                && self
                    .resolve_user_function_capture_hidden_name(scope_name)
                    .is_none()
            {
                bindings.insert(
                    name.clone(),
                    Expression::Member {
                        object: Box::new(scope_object),
                        property: Box::new(Expression::String(name)),
                    },
                );
                continue;
            }
            let hidden_name = self.allocate_named_hidden_local(
                "capture_scope",
                self.infer_value_kind(&scope_object)
                    .unwrap_or(StaticValueKind::Object),
            );
            self.emit_numeric_expression(&scope_object)?;
            let hidden_local = self
                .state
                .runtime
                .locals
                .get(&hidden_name)
                .copied()
                .expect("hidden capture scope local should be allocated");
            self.push_local_set(hidden_local);
            self.update_capture_slot_binding_from_expression(&hidden_name, &scope_object)?;
            bindings.insert(
                name.clone(),
                Expression::Member {
                    object: Box::new(Expression::Identifier(hidden_name)),
                    property: Box::new(Expression::String(name)),
                },
            );
        }

        Ok(Some(SpecializedFunctionValue {
            binding: template.binding.clone(),
            summary: rewrite_inline_function_summary_bindings(&template.summary, &bindings),
        }))
    }
}
