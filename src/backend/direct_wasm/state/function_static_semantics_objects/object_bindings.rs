use super::FunctionObjectSemanticsState;
use crate::backend::direct_wasm::{
    ObjectValueBinding, PropertyDescriptorBinding, empty_object_value_binding,
};
use std::collections::HashMap;

impl FunctionObjectSemanticsState {
    pub(in crate::backend::direct_wasm) fn local_object_binding_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut ObjectValueBinding> {
        self.local_object_bindings.get_mut(name)
    }

    pub(in crate::backend::direct_wasm) fn local_object_binding(
        &self,
        name: &str,
    ) -> Option<&ObjectValueBinding> {
        self.local_object_bindings.get(name)
    }

    pub(in crate::backend::direct_wasm) fn local_object_bindings_snapshot(
        &self,
    ) -> HashMap<String, ObjectValueBinding> {
        self.local_object_bindings.clone()
    }

    pub(in crate::backend::direct_wasm) fn local_descriptor_bindings_snapshot(
        &self,
    ) -> HashMap<String, PropertyDescriptorBinding> {
        self.local_descriptor_bindings.clone()
    }

    pub(in crate::backend::direct_wasm) fn set_local_object_binding(
        &mut self,
        name: &str,
        object: ObjectValueBinding,
    ) {
        self.local_object_bindings.insert(name.to_string(), object);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_object_binding(&mut self, name: &str) {
        self.local_object_bindings.remove(name);
    }

    pub(in crate::backend::direct_wasm) fn ensure_local_object_binding(
        &mut self,
        name: &str,
    ) -> &mut ObjectValueBinding {
        self.local_object_bindings
            .entry(name.to_string())
            .or_insert_with(empty_object_value_binding)
    }
}
