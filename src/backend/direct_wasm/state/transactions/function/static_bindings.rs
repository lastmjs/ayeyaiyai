use super::super::super::function_semantics::{
    FunctionArraySemanticsState, FunctionObjectSemanticsState, FunctionValueSemanticsState,
};
use super::super::super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct FunctionStaticBindingMetadataSnapshot {
    pub(in crate::backend::direct_wasm) values: FunctionValueSemanticsState,
    pub(in crate::backend::direct_wasm) objects: FunctionObjectSemanticsState,
    pub(in crate::backend::direct_wasm) arrays: FunctionArraySemanticsState,
    pub(in crate::backend::direct_wasm) materializing_expression_keys: HashSet<usize>,
    pub(in crate::backend::direct_wasm) eval_lexical_initialized_locals: HashMap<String, u32>,
    pub(in crate::backend::direct_wasm) capture_slot_source_bindings: HashMap<String, String>,
    pub(in crate::backend::direct_wasm) last_bound_user_function_call:
        Option<BoundUserFunctionCallSnapshot>,
}

pub(in crate::backend::direct_wasm) struct FunctionStaticBindingMetadataTransaction {
    pub(in crate::backend::direct_wasm) binding_metadata: FunctionStaticBindingMetadataSnapshot,
}

impl FunctionStaticBindingMetadataTransaction {
    pub(in crate::backend::direct_wasm) fn capture(
        state: &FunctionCompilerState,
    ) -> FunctionStaticBindingMetadataTransaction {
        FunctionStaticBindingMetadataTransaction {
            binding_metadata: state.snapshot_static_binding_metadata(),
        }
    }

    pub(in crate::backend::direct_wasm) fn restore(self, state: &mut FunctionCompilerState) {
        state.restore_static_binding_metadata(self.binding_metadata);
    }
}

#[derive(Clone, Default)]
pub(in crate::backend::direct_wasm) struct LocalStaticBindingState {
    pub(in crate::backend::direct_wasm) value: Option<Expression>,
    pub(in crate::backend::direct_wasm) array: Option<ArrayValueBinding>,
    pub(in crate::backend::direct_wasm) object: Option<ObjectValueBinding>,
    pub(in crate::backend::direct_wasm) kind: Option<StaticValueKind>,
}

pub(in crate::backend::direct_wasm) struct LocalStaticBindingSnapshot {
    pub(in crate::backend::direct_wasm) name: String,
    pub(in crate::backend::direct_wasm) binding: LocalStaticBindingState,
}
