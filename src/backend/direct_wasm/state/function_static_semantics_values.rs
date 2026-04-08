use crate::backend::direct_wasm::{
    LocalFunctionBinding, ProxyValueBinding, SpecializedFunctionValue, StaticValueKind,
};
use crate::ir::hir::Expression;
use std::collections::HashMap;

#[path = "function_static_semantics_values/cleanup.rs"]
mod cleanup;
#[path = "function_static_semantics_values/function_bindings.rs"]
mod function_bindings;
#[path = "function_static_semantics_values/proxy_bindings.rs"]
mod proxy_bindings;
#[path = "function_static_semantics_values/value_bindings.rs"]
mod value_bindings;

#[derive(Clone, Default)]
pub(in crate::backend::direct_wasm) struct FunctionValueSemanticsState {
    pub(in crate::backend::direct_wasm) local_kinds: HashMap<String, StaticValueKind>,
    pub(in crate::backend::direct_wasm) local_value_bindings: HashMap<String, Expression>,
    pub(in crate::backend::direct_wasm) local_function_bindings:
        HashMap<String, LocalFunctionBinding>,
    pub(in crate::backend::direct_wasm) local_specialized_function_values:
        HashMap<String, SpecializedFunctionValue>,
    pub(in crate::backend::direct_wasm) local_proxy_bindings: HashMap<String, ProxyValueBinding>,
}

impl FunctionValueSemanticsState {
    pub(in crate::backend::direct_wasm) fn from_prepared_bindings(
        local_kinds: HashMap<String, StaticValueKind>,
        local_value_bindings: HashMap<String, Expression>,
        local_function_bindings: HashMap<String, LocalFunctionBinding>,
    ) -> Self {
        Self {
            local_kinds,
            local_value_bindings,
            local_function_bindings,
            local_specialized_function_values: HashMap::new(),
            local_proxy_bindings: HashMap::new(),
        }
    }
}
