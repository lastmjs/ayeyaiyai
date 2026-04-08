use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn ensure_runtime_array_length_local(
        &mut self,
        name: &str,
    ) -> u32 {
        if let Some(local) = self
            .state
            .speculation
            .static_semantics
            .runtime_array_length_local(name)
        {
            return local;
        }
        let local = self.allocate_temp_local();
        self.state
            .speculation
            .static_semantics
            .set_runtime_array_length_local(name, local);
        local
    }

    pub(in crate::backend::direct_wasm) fn resolve_runtime_array_binding_name(
        &self,
        name: &str,
    ) -> Option<String> {
        if self
            .state
            .speculation
            .static_semantics
            .has_local_array_binding(name)
            || self
                .state
                .speculation
                .static_semantics
                .runtime_array_length_local(name)
                .is_some()
            || self
                .state
                .speculation
                .static_semantics
                .has_runtime_array_slots(name)
        {
            return Some(name.to_string());
        }
        let (resolved_name, _) = self.resolve_current_local_binding(name)?;
        if self
            .state
            .speculation
            .static_semantics
            .has_local_array_binding(&resolved_name)
            || self
                .state
                .speculation
                .static_semantics
                .runtime_array_length_local(&resolved_name)
                .is_some()
            || self
                .state
                .speculation
                .static_semantics
                .has_runtime_array_slots(&resolved_name)
        {
            return Some(resolved_name);
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_local_array_iterator_binding_name(
        &self,
        name: &str,
    ) -> Option<String> {
        if self
            .state
            .speculation
            .static_semantics
            .has_local_array_iterator_binding(name)
        {
            return Some(name.to_string());
        }
        let (resolved_name, _) = self.resolve_current_local_binding(name)?;
        self.state
            .speculation
            .static_semantics
            .has_local_array_iterator_binding(&resolved_name)
            .then_some(resolved_name)
    }

    pub(in crate::backend::direct_wasm) fn runtime_array_length_local_for_expression(
        &self,
        expression: &Expression,
    ) -> Option<u32> {
        let Expression::Identifier(name) = expression else {
            return None;
        };
        let binding_name = self
            .resolve_runtime_array_binding_name(name)
            .unwrap_or_else(|| name.clone());
        self.state
            .speculation
            .static_semantics
            .runtime_array_length_local(&binding_name)
    }

    pub(in crate::backend::direct_wasm) fn ensure_runtime_array_slots_for_binding(
        &mut self,
        name: &str,
        binding: &ArrayValueBinding,
    ) {
        for index in 0..TRACKED_ARRAY_SLOT_LIMIT {
            let slot = if let Some(slot) = self.runtime_array_slot(name, index) {
                slot
            } else {
                let slot = RuntimeArraySlot {
                    value_local: self.allocate_temp_local(),
                    present_local: self.allocate_temp_local(),
                };
                self.state
                    .speculation
                    .static_semantics
                    .set_runtime_array_slot(name, index, slot.clone());
                slot
            };
            match binding.values.get(index as usize).cloned().flatten() {
                Some(value) => {
                    self.emit_numeric_expression(&value)
                        .expect("runtime array slot initialization is supported");
                    self.push_local_set(slot.value_local);
                    self.push_i32_const(1);
                    self.push_local_set(slot.present_local);
                }
                None => {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    self.push_local_set(slot.value_local);
                    self.push_i32_const(0);
                    self.push_local_set(slot.present_local);
                }
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn runtime_array_slot(
        &self,
        name: &str,
        index: u32,
    ) -> Option<RuntimeArraySlot> {
        self.state
            .speculation
            .static_semantics
            .runtime_array_slot(name, index)
    }
}
