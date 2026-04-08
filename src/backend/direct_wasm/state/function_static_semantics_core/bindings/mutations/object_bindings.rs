use super::super::super::FunctionStaticSemanticsState;
use crate::backend::direct_wasm::ObjectValueBinding;

impl FunctionStaticSemanticsState {
    pub(in crate::backend::direct_wasm) fn set_local_object_binding(
        &mut self,
        name: &str,
        object: ObjectValueBinding,
    ) {
        self.objects.set_local_object_binding(name, object);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_object_binding(&mut self, name: &str) {
        self.objects.clear_local_object_binding(name);
    }

    pub(in crate::backend::direct_wasm) fn ensure_local_object_binding(
        &mut self,
        name: &str,
    ) -> &mut ObjectValueBinding {
        self.objects.ensure_local_object_binding(name)
    }
}
