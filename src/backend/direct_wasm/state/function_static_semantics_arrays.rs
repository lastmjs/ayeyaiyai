use crate::backend::direct_wasm::{
    ArrayIteratorBinding, ArrayValueBinding, IteratorStepBinding, ResizableArrayBufferBinding,
    RuntimeArraySlot, SpecializedFunctionValue, TypedArrayViewBinding,
};
use std::collections::HashMap;

#[path = "function_static_semantics_arrays/array_bindings.rs"]
mod array_bindings;
#[path = "function_static_semantics_arrays/cleanup.rs"]
mod cleanup;
#[path = "function_static_semantics_arrays/iterators.rs"]
mod iterators;
#[path = "function_static_semantics_arrays/runtime_slots.rs"]
mod runtime_slots;
#[path = "function_static_semantics_arrays/typed_arrays.rs"]
mod typed_arrays;

#[derive(Clone, Default)]
pub(in crate::backend::direct_wasm) struct FunctionArraySemanticsState {
    pub(in crate::backend::direct_wasm) local_array_bindings: HashMap<String, ArrayValueBinding>,
    pub(in crate::backend::direct_wasm) local_resizable_array_buffer_bindings:
        HashMap<String, ResizableArrayBufferBinding>,
    pub(in crate::backend::direct_wasm) local_typed_array_view_bindings:
        HashMap<String, TypedArrayViewBinding>,
    pub(in crate::backend::direct_wasm) runtime_typed_array_oob_locals: HashMap<String, u32>,
    pub(in crate::backend::direct_wasm) tracked_array_function_values:
        HashMap<String, HashMap<u32, SpecializedFunctionValue>>,
    pub(in crate::backend::direct_wasm) runtime_array_slots:
        HashMap<String, HashMap<u32, RuntimeArraySlot>>,
    pub(in crate::backend::direct_wasm) local_array_iterator_bindings:
        HashMap<String, ArrayIteratorBinding>,
    pub(in crate::backend::direct_wasm) local_iterator_step_bindings:
        HashMap<String, IteratorStepBinding>,
    pub(in crate::backend::direct_wasm) runtime_array_length_locals: HashMap<String, u32>,
}

impl FunctionArraySemanticsState {
    pub(in crate::backend::direct_wasm) fn from_prepared_bindings(
        local_array_bindings: HashMap<String, ArrayValueBinding>,
    ) -> Self {
        Self {
            local_array_bindings,
            local_resizable_array_buffer_bindings: HashMap::new(),
            local_typed_array_view_bindings: HashMap::new(),
            runtime_typed_array_oob_locals: HashMap::new(),
            tracked_array_function_values: HashMap::new(),
            runtime_array_slots: HashMap::new(),
            local_array_iterator_bindings: HashMap::new(),
            local_iterator_step_bindings: HashMap::new(),
            runtime_array_length_locals: HashMap::new(),
        }
    }
}
