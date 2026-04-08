use super::super::super::super::*;
use crate::backend::direct_wasm::GlobalValueService;

impl GlobalValueService {
    pub(in crate::backend::direct_wasm) fn upsert_property_descriptor(
        &mut self,
        name: String,
        state: GlobalPropertyDescriptorState,
    ) {
        match self.property_descriptors.get_mut(&name) {
            Some(existing) => *existing = state,
            None => {
                self.property_descriptors.insert(name, state);
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn clear_property_descriptor(&mut self, name: &str) {
        self.property_descriptors.remove(name);
    }
}
