use super::super::super::super::*;
use super::super::super::StaticResolutionEnvironment;

impl StaticResolutionEnvironment {
    pub(in crate::backend::direct_wasm) fn object_binding_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut ObjectValueBinding> {
        if self.local_object_bindings.contains_key(name) {
            return self.local_object_bindings.get_mut(name);
        }
        if !self.global_object_overrides.contains_key(name)
            && let Some(binding) = self.global_object_bindings.get(name).cloned()
        {
            self.global_object_overrides
                .insert(name.to_string(), Some(binding));
        }
        self.global_object_overrides
            .get_mut(name)
            .and_then(|binding| binding.as_mut())
    }

    pub(in crate::backend::direct_wasm) fn set_object_binding(
        &mut self,
        name: String,
        binding: ObjectValueBinding,
    ) {
        if self.local_bindings.contains_key(&name) || self.local_object_bindings.contains_key(&name)
        {
            self.local_object_bindings.insert(name, binding);
        } else {
            self.global_object_overrides.insert(name, Some(binding));
        }
    }

    pub(in crate::backend::direct_wasm) fn set_local_object_binding(
        &mut self,
        name: String,
        binding: ObjectValueBinding,
    ) {
        self.local_object_bindings.insert(name, binding);
    }

    pub(in crate::backend::direct_wasm) fn clear_object_binding(&mut self, name: &str) {
        if self.local_object_bindings.contains_key(name) {
            self.local_object_bindings.remove(name);
        } else {
            self.global_object_overrides.insert(name.to_string(), None);
        }
    }

    pub(in crate::backend::direct_wasm) fn sync_object_binding(
        &mut self,
        name: &str,
        binding: Option<ObjectValueBinding>,
    ) {
        if let Some(binding) = binding {
            self.set_object_binding(name.to_string(), binding);
        } else {
            self.clear_object_binding(name);
        }
    }
}
