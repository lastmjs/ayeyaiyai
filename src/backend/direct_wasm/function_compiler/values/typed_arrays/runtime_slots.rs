use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn ensure_runtime_array_slot_entry(
        &mut self,
        name: &str,
        index: u32,
    ) -> RuntimeArraySlot {
        if let Some(slot) = self.runtime_array_slot(name, index) {
            return slot;
        }
        let slot = RuntimeArraySlot {
            value_local: self.allocate_temp_local(),
            present_local: self.allocate_temp_local(),
        };
        self.state
            .speculation
            .static_semantics
            .set_runtime_array_slot(name, index, slot.clone());
        slot
    }

    pub(in crate::backend::direct_wasm) fn typed_array_oob_local(&mut self, name: &str) -> u32 {
        if let Some(local) = self
            .state
            .speculation
            .static_semantics
            .runtime_typed_array_oob_local(name)
        {
            return local;
        }
        let local = self.allocate_temp_local();
        self.state
            .speculation
            .static_semantics
            .set_runtime_typed_array_oob_local(name, local);
        local
    }
}
