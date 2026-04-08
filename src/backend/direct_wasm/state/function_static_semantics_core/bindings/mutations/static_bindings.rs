use super::super::super::FunctionStaticSemanticsState;
use crate::backend::direct_wasm::{
    ArrayValueBinding, FunctionStaticBindingMetadataSnapshot, LocalStaticBindingSnapshot,
    LocalStaticBindingState, ObjectValueBinding, StaticValueKind,
};
use crate::ir::hir::Expression;

impl FunctionStaticSemanticsState {
    pub(in crate::backend::direct_wasm) fn restore_static_binding_metadata(
        &mut self,
        snapshot: FunctionStaticBindingMetadataSnapshot,
    ) {
        self.values = snapshot.values;
        self.objects = snapshot.objects;
        self.arrays = snapshot.arrays;
        *self.materializing_expression_keys.borrow_mut() = snapshot.materializing_expression_keys;
        self.eval_lexical_initialized_locals = snapshot.eval_lexical_initialized_locals;
        self.capture_slot_source_bindings = snapshot.capture_slot_source_bindings;
        self.last_bound_user_function_call = snapshot.last_bound_user_function_call;
    }

    pub(in crate::backend::direct_wasm) fn apply_local_static_binding_state(
        &mut self,
        name: &str,
        binding: LocalStaticBindingState,
    ) {
        let LocalStaticBindingState {
            value,
            array,
            object,
            kind,
        } = binding;

        if let Some(value) = value {
            self.set_local_value_binding(name, value);
        } else {
            self.clear_local_value_binding(name);
        }

        if let Some(array) = array {
            self.set_local_array_binding(name, array);
        } else {
            self.clear_local_array_binding(name);
        }

        if let Some(object) = object {
            self.set_local_object_binding(name, object);
        } else {
            self.clear_local_object_binding(name);
        }

        if let Some(kind) = kind {
            self.set_local_kind(name, kind);
        } else {
            self.clear_local_kind(name);
        }
    }

    pub(in crate::backend::direct_wasm) fn set_local_static_binding(
        &mut self,
        name: &str,
        value: Expression,
        array: Option<ArrayValueBinding>,
        object: Option<ObjectValueBinding>,
        kind: Option<StaticValueKind>,
    ) {
        self.apply_local_static_binding_state(
            name,
            LocalStaticBindingState {
                value: Some(value),
                array,
                object,
                kind,
            },
        );
    }

    pub(in crate::backend::direct_wasm) fn restore_local_static_binding(
        &mut self,
        snapshot: LocalStaticBindingSnapshot,
    ) {
        self.apply_local_static_binding_state(&snapshot.name, snapshot.binding);
    }
}
