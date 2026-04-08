use super::super::super::super::*;
use crate::backend::direct_wasm::GlobalValueService;

impl GlobalValueService {
    pub(in crate::backend::direct_wasm) fn array_uses_runtime_state(&self, name: &str) -> bool {
        self.arrays_with_runtime_state.contains(name)
    }

    pub(in crate::backend::direct_wasm) fn prototype_object_binding(
        &self,
        name: &str,
    ) -> Option<&ObjectValueBinding> {
        self.prototype_object_bindings.get(name)
    }

    pub(in crate::backend::direct_wasm) fn has_prototype_object_binding(&self, name: &str) -> bool {
        self.prototype_object_bindings.contains_key(name)
    }

    pub(in crate::backend::direct_wasm) fn object_prototype_expression(
        &self,
        name: &str,
    ) -> Option<&Expression> {
        self.object_prototype_bindings.get(name)
    }

    pub(in crate::backend::direct_wasm) fn runtime_prototype_binding_count(&self) -> u32 {
        self.runtime_prototype_bindings.len() as u32
    }

    pub(in crate::backend::direct_wasm) fn runtime_prototype_binding_names(&self) -> Vec<String> {
        self.runtime_prototype_bindings.keys().cloned().collect()
    }

    pub(in crate::backend::direct_wasm) fn max_runtime_prototype_global_index(
        &self,
    ) -> Option<u32> {
        self.runtime_prototype_bindings
            .values()
            .filter_map(|binding| binding.global_index)
            .max()
    }

    pub(in crate::backend::direct_wasm) fn runtime_prototype_binding(
        &self,
        name: &str,
    ) -> Option<&GlobalObjectRuntimePrototypeBinding> {
        self.runtime_prototype_bindings.get(name)
    }
}
