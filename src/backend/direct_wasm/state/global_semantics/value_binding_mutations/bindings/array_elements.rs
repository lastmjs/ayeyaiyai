use super::super::super::super::super::*;
use crate::backend::direct_wasm::GlobalValueService;

impl GlobalValueService {
    pub(in crate::backend::direct_wasm) fn set_array_element_binding(
        &mut self,
        name: &str,
        index: usize,
        value: Expression,
    ) -> bool {
        let Some(array_binding) = self.array_binding_mut(name) else {
            return false;
        };
        while array_binding.values.len() <= index {
            array_binding.values.push(None);
        }
        array_binding.values[index] = Some(value);
        true
    }
}
