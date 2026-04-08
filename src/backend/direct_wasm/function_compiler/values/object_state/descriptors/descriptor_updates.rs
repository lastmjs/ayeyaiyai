use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn update_local_descriptor_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Some(descriptor_binding) = self.resolve_descriptor_binding_from_expression(value)
        else {
            self.state
                .speculation
                .static_semantics
                .objects
                .local_descriptor_bindings
                .remove(name);
            return;
        };
        self.state
            .speculation
            .static_semantics
            .objects
            .local_descriptor_bindings
            .insert(name.to_string(), descriptor_binding);
        self.state
            .speculation
            .static_semantics
            .set_local_kind(name, StaticValueKind::Object);
    }

    pub(in crate::backend::direct_wasm) fn update_global_property_descriptor_value(
        &mut self,
        name: &str,
        value_expression: &Expression,
    ) {
        let materialized = self
            .backend
            .global_value_binding(name)
            .cloned()
            .unwrap_or_else(|| self.materialize_static_expression(value_expression));
        if let Some(mut state) = self.backend.global_property_descriptor(name).cloned() {
            state.value = materialized;
            self.backend
                .upsert_global_property_descriptor(name.to_string(), state);
        }
    }

    pub(in crate::backend::direct_wasm) fn ensure_global_property_descriptor_value(
        &mut self,
        name: &str,
        value_expression: &Expression,
        configurable: bool,
    ) {
        let materialized = self
            .backend
            .global_value_binding(name)
            .cloned()
            .unwrap_or_else(|| self.materialize_static_expression(value_expression));
        let next_state = self
            .backend
            .global_property_descriptor(name)
            .cloned()
            .map(|mut state| {
                state.value = materialized.clone();
                state
            })
            .unwrap_or(GlobalPropertyDescriptorState {
                value: materialized,
                writable: Some(true),
                enumerable: true,
                configurable,
            });
        self.backend
            .upsert_global_property_descriptor(name.to_string(), next_state);
    }

    pub(in crate::backend::direct_wasm) fn instantiate_eval_global_function_property_descriptor(
        &mut self,
        name: &str,
    ) {
        let value = Expression::Identifier(name.to_string());
        let next_state = match self.backend.global_property_descriptor(name).cloned() {
            Some(mut state) if !state.configurable => {
                state.value = value;
                state
            }
            Some(_) | None => GlobalPropertyDescriptorState {
                value,
                writable: Some(true),
                enumerable: true,
                configurable: true,
            },
        };
        self.backend
            .upsert_global_property_descriptor(name.to_string(), next_state);
    }

    pub(in crate::backend::direct_wasm) fn update_local_value_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let snapshot_value = self
            .state
            .speculation
            .static_semantics
            .local_value_binding(name)
            .or_else(|| self.global_value_binding(name))
            .map(|snapshot| substitute_self_referential_binding_snapshot(value, name, snapshot))
            .unwrap_or_else(|| value.clone());
        let mut referenced_names = HashSet::new();
        collect_referenced_binding_names_from_expression(&snapshot_value, &mut referenced_names);
        if referenced_names.contains(name) {
            self.state
                .speculation
                .static_semantics
                .clear_local_value_binding(name);
            return;
        }
        let materialized_value =
            if let Some(bigint) = self.resolve_static_bigint_value(&snapshot_value) {
                Expression::BigInt(bigint.to_string())
            } else {
                self.resolve_static_string_value(&snapshot_value)
                    .map(Expression::String)
                    .unwrap_or_else(|| self.materialize_static_expression(&snapshot_value))
            };
        self.state
            .speculation
            .static_semantics
            .set_local_value_binding(name, materialized_value);
    }
}
