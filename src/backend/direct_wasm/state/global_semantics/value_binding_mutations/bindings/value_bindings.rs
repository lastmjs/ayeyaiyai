use super::super::super::super::super::*;
use crate::backend::direct_wasm::GlobalValueService;

impl GlobalValueService {
    pub(in crate::backend::direct_wasm) fn clear_value_binding(&mut self, name: &str) {
        self.value_bindings.remove(name);
    }

    pub(in crate::backend::direct_wasm) fn set_value_binding(
        &mut self,
        name: String,
        value: Expression,
    ) {
        self.value_bindings.insert(name, value);
    }
}
