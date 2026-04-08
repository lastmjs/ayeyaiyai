use super::super::super::super::*;
use crate::backend::direct_wasm::GlobalValueService;

impl GlobalValueService {
    pub(in crate::backend::direct_wasm) fn property_descriptor(
        &self,
        name: &str,
    ) -> Option<&GlobalPropertyDescriptorState> {
        self.property_descriptors.get(name)
    }
}
