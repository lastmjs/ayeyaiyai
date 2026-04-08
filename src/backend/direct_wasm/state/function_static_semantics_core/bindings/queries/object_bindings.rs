use super::super::super::FunctionStaticSemanticsState;
use crate::backend::direct_wasm::ObjectValueBinding;

impl FunctionStaticSemanticsState {
    pub(in crate::backend::direct_wasm) fn local_object_binding_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut ObjectValueBinding> {
        self.objects.local_object_binding_mut(name)
    }

    pub(in crate::backend::direct_wasm) fn local_object_binding(
        &self,
        name: &str,
    ) -> Option<&ObjectValueBinding> {
        self.objects.local_object_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn has_local_object_binding(&self, name: &str) -> bool {
        self.local_object_binding(name).is_some()
    }
}
