use super::super::super::*;
use super::{
    function_static_semantics_arrays::FunctionArraySemanticsState,
    function_static_semantics_objects::FunctionObjectSemanticsState,
    function_static_semantics_values::FunctionValueSemanticsState,
};

mod bindings;
mod cleanup;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct FunctionStaticSemanticsState {
    pub(in crate::backend::direct_wasm) values: FunctionValueSemanticsState,
    pub(in crate::backend::direct_wasm) objects: FunctionObjectSemanticsState,
    pub(in crate::backend::direct_wasm) arrays: FunctionArraySemanticsState,
    pub(in crate::backend::direct_wasm) materializing_expression_keys: RefCell<HashSet<usize>>,
    pub(in crate::backend::direct_wasm) eval_lexical_initialized_locals: HashMap<String, u32>,
    pub(in crate::backend::direct_wasm) capture_slot_source_bindings: HashMap<String, String>,
    pub(in crate::backend::direct_wasm) last_bound_user_function_call:
        Option<BoundUserFunctionCallSnapshot>,
}

impl FunctionStaticSemanticsState {
    pub(in crate::backend::direct_wasm) fn from_prepared_bindings(
        bindings: PreparedLocalStaticBindings,
    ) -> Self {
        let PreparedLocalStaticBindings {
            local_kinds,
            local_value_bindings,
            local_function_bindings,
            local_array_bindings,
            local_object_bindings,
        } = bindings;
        Self {
            values: FunctionValueSemanticsState::from_prepared_bindings(
                local_kinds,
                local_value_bindings,
                local_function_bindings,
            ),
            objects: FunctionObjectSemanticsState::from_prepared_bindings(local_object_bindings),
            arrays: FunctionArraySemanticsState::from_prepared_bindings(local_array_bindings),
            materializing_expression_keys: RefCell::new(HashSet::new()),
            eval_lexical_initialized_locals: HashMap::new(),
            capture_slot_source_bindings: HashMap::new(),
            last_bound_user_function_call: None,
        }
    }
}
