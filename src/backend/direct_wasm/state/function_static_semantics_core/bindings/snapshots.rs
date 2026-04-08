use super::super::FunctionStaticSemanticsState;
use crate::backend::direct_wasm::{
    FunctionStaticBindingMetadataSnapshot, LocalStaticBindingSnapshot, LocalStaticBindingState,
    SharedGlobalBindingEnvironment, StaticResolutionEnvironment,
};
use crate::ir::hir::Expression;
use std::collections::HashMap;

impl FunctionStaticSemanticsState {
    pub(in crate::backend::direct_wasm) fn local_static_binding_state(
        &self,
        name: &str,
    ) -> LocalStaticBindingState {
        LocalStaticBindingState {
            value: self.values.local_value_binding(name).cloned(),
            array: self.arrays.local_array_binding(name).cloned(),
            object: self.objects.local_object_binding(name).cloned(),
            kind: self.values.local_kind(name),
        }
    }

    pub(in crate::backend::direct_wasm) fn snapshot_local_static_binding(
        &self,
        name: &str,
    ) -> LocalStaticBindingSnapshot {
        LocalStaticBindingSnapshot {
            name: name.to_string(),
            binding: self.local_static_binding_state(name),
        }
    }

    pub(in crate::backend::direct_wasm) fn snapshot_static_binding_metadata(
        &self,
    ) -> FunctionStaticBindingMetadataSnapshot {
        FunctionStaticBindingMetadataSnapshot {
            values: self.values.clone(),
            objects: self.objects.clone(),
            arrays: self.arrays.clone(),
            materializing_expression_keys: self.materializing_expression_keys.borrow().clone(),
            eval_lexical_initialized_locals: self.eval_lexical_initialized_locals.clone(),
            capture_slot_source_bindings: self.capture_slot_source_bindings.clone(),
            last_bound_user_function_call: self.last_bound_user_function_call.clone(),
        }
    }

    pub(in crate::backend::direct_wasm) fn snapshot_static_resolution_environment(
        &self,
        global_bindings: &SharedGlobalBindingEnvironment,
    ) -> StaticResolutionEnvironment {
        self.snapshot_static_resolution_environment_with_local_bindings(
            global_bindings,
            self.values.local_value_bindings_snapshot(),
        )
    }

    pub(in crate::backend::direct_wasm) fn snapshot_static_resolution_environment_with_local_bindings(
        &self,
        global_bindings: &SharedGlobalBindingEnvironment,
        local_bindings: HashMap<String, Expression>,
    ) -> StaticResolutionEnvironment {
        StaticResolutionEnvironment::from_binding_snapshots(
            global_bindings.value_bindings.clone(),
            global_bindings.object_bindings.clone(),
            local_bindings,
            self.objects.local_object_bindings_snapshot(),
            self.objects.local_descriptor_bindings_snapshot(),
        )
    }
}
