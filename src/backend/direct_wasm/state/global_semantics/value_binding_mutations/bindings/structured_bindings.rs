use super::super::super::super::super::*;
use crate::backend::direct_wasm::GlobalValueService;

impl GlobalValueService {
    pub(in crate::backend::direct_wasm) fn sync_array_binding(
        &mut self,
        name: &str,
        binding: Option<ArrayValueBinding>,
    ) {
        if let Some(binding) = binding {
            self.array_bindings.insert(name.to_string(), binding);
        } else {
            self.array_bindings.remove(name);
        }
    }

    pub(in crate::backend::direct_wasm) fn sync_object_binding(
        &mut self,
        name: &str,
        binding: Option<ObjectValueBinding>,
    ) {
        if let Some(binding) = binding {
            self.object_bindings.insert(name.to_string(), binding);
        } else {
            self.object_bindings.remove(name);
        }
    }

    pub(in crate::backend::direct_wasm) fn sync_arguments_binding(
        &mut self,
        name: &str,
        binding: Option<ArgumentsValueBinding>,
    ) {
        if let Some(binding) = binding {
            self.arguments_bindings.insert(name.to_string(), binding);
        } else {
            self.arguments_bindings.remove(name);
        }
    }
}
