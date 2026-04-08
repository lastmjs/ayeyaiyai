use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn update_local_array_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Some(array_binding) = self.resolve_array_binding_from_expression(value) else {
            self.state
                .speculation
                .static_semantics
                .clear_local_array_binding(name);
            self.state
                .speculation
                .static_semantics
                .clear_runtime_array_slots(name);
            self.state
                .speculation
                .static_semantics
                .clear_tracked_array_specialized_function_values(name);
            return;
        };
        let source_binding_name = if let Expression::Identifier(source_name) = value {
            self.resolve_runtime_array_binding_name(source_name)
        } else {
            None
        };
        let copy_internal_rest_runtime_state = source_binding_name
            .as_ref()
            .is_some_and(|source_name| source_name.starts_with("__ayy_array_rest_"));
        let length_local = if copy_internal_rest_runtime_state {
            self.ensure_runtime_array_length_local(name)
        } else if let Some(source_name) = source_binding_name.as_ref() {
            self.state
                .speculation
                .static_semantics
                .runtime_array_length_local(source_name)
                .unwrap_or_else(|| self.ensure_runtime_array_length_local(name))
        } else {
            self.ensure_runtime_array_length_local(name)
        };
        self.state
            .speculation
            .static_semantics
            .set_runtime_array_length_local(name, length_local);
        if copy_internal_rest_runtime_state {
            let source_name = source_binding_name
                .as_ref()
                .expect("rest runtime copy should have a source binding");
            if let Some(source_length_local) = self
                .state
                .speculation
                .static_semantics
                .runtime_array_length_local(source_name)
            {
                self.push_local_get(source_length_local);
            } else {
                self.push_i32_const(array_binding.values.len() as i32);
            }
        } else if let Some(source_length_local) =
            self.runtime_array_length_local_for_expression(value)
        {
            self.push_local_get(source_length_local);
        } else {
            self.push_i32_const(array_binding.values.len() as i32);
        }
        self.push_local_set(length_local);
        if copy_internal_rest_runtime_state {
            let source_name = source_binding_name
                .as_ref()
                .expect("rest runtime copy should have a source binding");
            for index in 0..TRACKED_ARRAY_SLOT_LIMIT {
                let target_slot = self.ensure_runtime_array_slot_entry(name, index);
                if let Some(source_slot) = self.runtime_array_slot(source_name, index) {
                    self.push_local_get(source_slot.value_local);
                    self.push_local_set(target_slot.value_local);
                    self.push_local_get(source_slot.present_local);
                    self.push_local_set(target_slot.present_local);
                } else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    self.push_local_set(target_slot.value_local);
                    self.push_i32_const(0);
                    self.push_local_set(target_slot.present_local);
                }
            }
        } else if let Some(source_name) = source_binding_name.as_ref() {
            if let Some(source_slots) = self
                .state
                .speculation
                .static_semantics
                .runtime_array_slots(source_name)
            {
                self.state
                    .speculation
                    .static_semantics
                    .set_runtime_array_slots(name, source_slots);
            } else {
                self.ensure_runtime_array_slots_for_binding(name, &array_binding);
            }
        } else {
            self.ensure_runtime_array_slots_for_binding(name, &array_binding);
        }
        self.state
            .speculation
            .static_semantics
            .set_local_array_binding(name, array_binding);
        if let Some(source_name) = source_binding_name.as_ref() {
            if let Some(bindings) = self
                .state
                .speculation
                .static_semantics
                .tracked_array_specialized_function_values(source_name)
            {
                self.state
                    .speculation
                    .static_semantics
                    .set_tracked_array_specialized_function_values(name, bindings);
            } else {
                self.state
                    .speculation
                    .static_semantics
                    .clear_tracked_array_specialized_function_values(name);
            }
        } else {
            self.state
                .speculation
                .static_semantics
                .clear_tracked_array_specialized_function_values(name);
        }
        self.state
            .speculation
            .static_semantics
            .set_local_kind(name, StaticValueKind::Object);
    }
}
