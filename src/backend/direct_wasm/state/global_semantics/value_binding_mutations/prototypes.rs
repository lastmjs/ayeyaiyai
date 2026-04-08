use super::super::super::super::*;
use crate::backend::direct_wasm::GlobalValueService;

impl GlobalValueService {
    pub(in crate::backend::direct_wasm) fn mark_array_with_runtime_state(&mut self, name: &str) {
        self.arrays_with_runtime_state.insert(name.to_string());
    }

    pub(in crate::backend::direct_wasm) fn sync_prototype_object_binding(
        &mut self,
        name: &str,
        binding: Option<ObjectValueBinding>,
    ) {
        if let Some(binding) = binding {
            self.prototype_object_bindings
                .insert(name.to_string(), binding);
        } else {
            self.clear_prototype_object_binding(name);
        }
    }

    pub(in crate::backend::direct_wasm) fn sync_object_prototype_expression(
        &mut self,
        name: &str,
        prototype: Option<Expression>,
    ) {
        if let Some(prototype) = prototype {
            self.object_prototype_bindings
                .insert(name.to_string(), prototype);
        } else {
            self.object_prototype_bindings.remove(name);
        }
    }

    pub(in crate::backend::direct_wasm) fn clear_prototype_object_binding(&mut self, name: &str) {
        self.prototype_object_bindings.remove(name);
    }

    pub(in crate::backend::direct_wasm) fn set_runtime_prototype_binding_global_index(
        &mut self,
        name: &str,
        global_index: u32,
    ) {
        if let Some(binding) = self.runtime_prototype_bindings.get_mut(name) {
            binding.global_index = Some(global_index);
        }
    }

    pub(in crate::backend::direct_wasm) fn record_runtime_prototype_variant(
        &mut self,
        name: &str,
        prototype: Option<Expression>,
    ) {
        let initial_variant = self.object_prototype_expression(name).cloned();
        let binding = self
            .runtime_prototype_bindings
            .entry(name.to_string())
            .or_insert_with(|| GlobalObjectRuntimePrototypeBinding {
                global_index: None,
                variants: vec![initial_variant],
            });
        if !binding
            .variants
            .iter()
            .any(|candidate| *candidate == prototype)
        {
            binding.variants.push(prototype);
        }
    }
}
