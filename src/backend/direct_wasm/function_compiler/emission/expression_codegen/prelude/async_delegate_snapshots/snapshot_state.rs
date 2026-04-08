use super::*;

#[derive(Clone, Default)]
struct VisibleRuntimeBindingSnapshot {
    bindings: HashMap<String, Expression>,
}

impl VisibleRuntimeBindingSnapshot {
    fn from_existing(bindings: &HashMap<String, Expression>) -> VisibleRuntimeBindingSnapshot {
        VisibleRuntimeBindingSnapshot {
            bindings: bindings.clone(),
        }
    }

    fn from_statements(
        compiler: &FunctionCompiler<'_>,
        statements: &[Statement],
    ) -> VisibleRuntimeBindingSnapshot {
        let bindings = collect_referenced_binding_names_from_statements(statements)
            .into_iter()
            .filter(|name| compiler.should_sync_async_delegate_snapshot_binding(name))
            .map(|name| {
                let identifier = Expression::Identifier(name.clone());
                (name, compiler.materialize_static_expression(&identifier))
            })
            .collect::<HashMap<_, _>>();
        VisibleRuntimeBindingSnapshot { bindings }
    }

    fn refresh_from_visible_state(&mut self, compiler: &FunctionCompiler<'_>) {
        let binding_names = self.bindings.keys().cloned().collect::<Vec<_>>();
        for name in binding_names {
            if !compiler.should_sync_async_delegate_snapshot_binding(&name)
                || name.starts_with("__ayy_async_delegate_")
            {
                continue;
            }
            let identifier = Expression::Identifier(name.clone());
            let refreshed = if let Some(array_binding) =
                compiler.resolve_array_binding_from_expression(&identifier)
            {
                Expression::Array(
                    array_binding
                        .values
                        .into_iter()
                        .map(|value| {
                            ArrayElement::Expression(value.unwrap_or(Expression::Undefined))
                        })
                        .collect(),
                )
            } else if let Some(object_binding) =
                compiler.resolve_object_binding_from_expression(&identifier)
            {
                object_binding_to_expression(&object_binding)
            } else if let Some(resolved) = compiler
                .resolve_bound_alias_expression(&identifier)
                .filter(|resolved| !static_expression_matches(resolved, &identifier))
            {
                compiler.materialize_static_expression(&resolved)
            } else {
                compiler.materialize_static_expression(&identifier)
            };
            self.bindings.insert(name, refreshed);
        }
    }

    fn sync_into_runtime(&self, compiler: &mut FunctionCompiler<'_>) -> DirectResult<()> {
        let mut visible_bindings = self
            .bindings
            .iter()
            .filter(|(name, _)| compiler.should_sync_async_delegate_snapshot_binding(name))
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect::<Vec<_>>();
        visible_bindings.sort_by(|left, right| left.0.cmp(&right.0));
        for (name, value) in visible_bindings {
            let synced_value = compiler.materialize_static_expression(&value);
            let value_local = compiler.allocate_temp_local();
            compiler.emit_numeric_expression(&synced_value)?;
            compiler.push_local_set(value_local);
            compiler.emit_store_identifier_value_local(&name, &synced_value, value_local)?;
            if let Some(array_binding) =
                compiler.resolve_array_binding_from_expression(&synced_value)
            {
                compiler.sync_named_global_runtime_array_from_binding(&name, &array_binding)?;
            }
        }
        Ok(())
    }

    fn push_scoped_shadow_bindings(
        &self,
        compiler: &mut FunctionCompiler<'_>,
    ) -> DirectResult<Vec<String>> {
        let mut binding_names = self.bindings.keys().cloned().collect::<Vec<_>>();
        binding_names.sort();
        let mut scoped_names = Vec::new();
        for name in binding_names {
            if compiler.should_sync_async_delegate_snapshot_binding(&name)
                || name.starts_with("__ayy_async_delegate_")
            {
                continue;
            }
            let Some(value) = self.bindings.get(&name) else {
                continue;
            };
            let scoped_name = compiler.allocate_named_hidden_local(
                &format!("async_delegate_snapshot_{name}"),
                compiler
                    .infer_value_kind(value)
                    .unwrap_or(StaticValueKind::Unknown),
            );
            let scoped_local = compiler
                .state
                .runtime
                .locals
                .get(&scoped_name)
                .copied()
                .expect("async delegate snapshot local must exist");
            let materialized_value = compiler.materialize_static_expression(value);
            compiler.emit_numeric_expression(&materialized_value)?;
            compiler.push_local_set(scoped_local);
            compiler
                .update_capture_slot_binding_from_expression(&scoped_name, &materialized_value)?;
            compiler
                .state
                .emission
                .lexical_scopes
                .active_scoped_lexical_bindings
                .entry(name.clone())
                .or_default()
                .push(scoped_name);
            scoped_names.push(name);
        }
        Ok(scoped_names)
    }

    fn into_bindings(self) -> HashMap<String, Expression> {
        self.bindings
    }
}

impl<'a> FunctionCompiler<'a> {
    fn sync_named_global_runtime_array_from_binding(
        &mut self,
        name: &str,
        binding: &ArrayValueBinding,
    ) -> DirectResult<()> {
        self.emit_sync_global_runtime_array_state_from_binding(name, binding)?;
        self.emit_force_global_runtime_array_state_from_binding(name, binding)?;
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn should_sync_async_delegate_snapshot_binding(
        &self,
        name: &str,
    ) -> bool {
        self.parameter_scope_arguments_local_for(name).is_some()
            || (self.is_current_arguments_binding_name(name) && self.has_arguments_object())
            || self.resolve_current_local_binding(name).is_some()
            || self.resolve_eval_local_function_hidden_name(name).is_some()
            || self
                .resolve_user_function_capture_hidden_name(name)
                .is_some()
            || self.global_has_binding(name)
            || self.global_has_implicit_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn sync_async_delegate_snapshot_bindings(
        &mut self,
        bindings: &HashMap<String, Expression>,
    ) -> DirectResult<()> {
        VisibleRuntimeBindingSnapshot::from_existing(bindings).sync_into_runtime(self)
    }

    pub(in crate::backend::direct_wasm) fn refresh_async_delegate_snapshot_bindings_from_visible_state(
        &self,
        bindings: &mut HashMap<String, Expression>,
    ) {
        let mut snapshot = VisibleRuntimeBindingSnapshot::from_existing(bindings);
        snapshot.refresh_from_visible_state(self);
        *bindings = snapshot.into_bindings();
    }

    pub(in crate::backend::direct_wasm) fn sync_visible_runtime_bindings_for_statements(
        &mut self,
        statements: &[Statement],
    ) -> DirectResult<()> {
        VisibleRuntimeBindingSnapshot::from_statements(self, statements).sync_into_runtime(self)
    }

    pub(in crate::backend::direct_wasm) fn push_async_delegate_snapshot_scope_bindings(
        &mut self,
        bindings: &HashMap<String, Expression>,
    ) -> DirectResult<Vec<String>> {
        VisibleRuntimeBindingSnapshot::from_existing(bindings).push_scoped_shadow_bindings(self)
    }

    pub(in crate::backend::direct_wasm) fn merge_last_bound_user_function_updated_bindings_into_snapshot(
        &mut self,
        function_name: &str,
        snapshot_bindings: &mut HashMap<String, Expression>,
    ) -> DirectResult<()> {
        let Some(updated_bindings) = self
            .state
            .speculation
            .static_semantics
            .last_bound_user_function_call
            .as_ref()
            .filter(|snapshot| snapshot.function_name == function_name)
            .map(|snapshot| snapshot.updated_bindings.clone())
        else {
            return Ok(());
        };
        for (name, value) in &updated_bindings {
            let source_name = scoped_binding_source_name(name).unwrap_or(name).to_string();
            let merged_value = snapshot_bindings
                .get(&source_name)
                .cloned()
                .and_then(|existing_value| {
                    let existing_binding =
                        self.resolve_array_binding_from_expression(&existing_value)?;
                    let updated_binding = self.resolve_array_binding_from_expression(value)?;
                    let existing_values = existing_binding
                        .values
                        .into_iter()
                        .map(|value| value.unwrap_or(Expression::Undefined))
                        .collect::<Vec<_>>();
                    let updated_values = updated_binding
                        .values
                        .into_iter()
                        .map(|value| value.unwrap_or(Expression::Undefined))
                        .collect::<Vec<_>>();
                    let merged_values = if updated_values.starts_with(&existing_values) {
                        updated_values
                    } else {
                        existing_values
                            .into_iter()
                            .chain(updated_values)
                            .collect::<Vec<_>>()
                    };
                    Some(Expression::Array(
                        merged_values
                            .into_iter()
                            .map(ArrayElement::Expression)
                            .collect(),
                    ))
                })
                .unwrap_or_else(|| value.clone());
            snapshot_bindings.insert(source_name.clone(), merged_value.clone());
            self.update_capture_slot_binding_from_expression(&source_name, &merged_value)?;
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn pop_async_delegate_snapshot_scope_bindings(
        &mut self,
        names: &[String],
    ) {
        for name in names.iter().rev() {
            self.state.pop_scoped_lexical_binding(name);
        }
    }
}
