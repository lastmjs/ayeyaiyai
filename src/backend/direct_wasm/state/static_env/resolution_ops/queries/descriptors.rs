use super::super::super::super::*;
use super::super::super::StaticResolutionEnvironment;

impl StaticResolutionEnvironment {
    pub(in crate::backend::direct_wasm) fn descriptor_binding(
        &self,
        name: &str,
    ) -> Option<&PropertyDescriptorBinding> {
        self.local_descriptor_bindings.get(name)
    }
}
