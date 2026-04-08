use super::super::super::super::*;
use super::super::super::StaticResolutionEnvironment;

impl StaticResolutionEnvironment {
    pub(in crate::backend::direct_wasm) fn object_binding(
        &self,
        name: &str,
    ) -> Option<&ObjectValueBinding> {
        self.local_object_bindings
            .get(name)
            .or_else(|| {
                self.global_object_overrides
                    .get(name)
                    .and_then(|binding| binding.as_ref())
            })
            .or_else(|| {
                (!self.global_object_overrides.contains_key(name))
                    .then(|| self.global_object_bindings.get(name))
                    .flatten()
            })
    }

    pub(in crate::backend::direct_wasm) fn contains_object_binding(&self, name: &str) -> bool {
        self.local_object_bindings.contains_key(name)
            || self
                .global_object_overrides
                .get(name)
                .map(|binding| binding.is_some())
                .unwrap_or_else(|| self.global_object_bindings.contains_key(name))
    }
}
