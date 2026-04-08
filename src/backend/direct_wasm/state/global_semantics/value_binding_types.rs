use super::super::super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct GlobalObjectRuntimePrototypeBinding {
    pub(in crate::backend::direct_wasm) global_index: Option<u32>,
    pub(in crate::backend::direct_wasm) variants: Vec<Option<Expression>>,
}

#[derive(Clone, Default)]
pub(in crate::backend::direct_wasm) struct GlobalValueService {
    pub(in crate::backend::direct_wasm) value_bindings: HashMap<String, Expression>,
    pub(in crate::backend::direct_wasm) array_bindings: HashMap<String, ArrayValueBinding>,
    pub(in crate::backend::direct_wasm) arrays_with_runtime_state: HashSet<String>,
    pub(in crate::backend::direct_wasm) object_bindings: HashMap<String, ObjectValueBinding>,
    pub(in crate::backend::direct_wasm) property_descriptors:
        HashMap<String, GlobalPropertyDescriptorState>,
    pub(in crate::backend::direct_wasm) object_prototype_bindings: HashMap<String, Expression>,
    pub(in crate::backend::direct_wasm) runtime_prototype_bindings:
        HashMap<String, GlobalObjectRuntimePrototypeBinding>,
    pub(in crate::backend::direct_wasm) prototype_object_bindings:
        HashMap<String, ObjectValueBinding>,
    pub(in crate::backend::direct_wasm) arguments_bindings: HashMap<String, ArgumentsValueBinding>,
    pub(in crate::backend::direct_wasm) proxy_bindings: HashMap<String, ProxyValueBinding>,
}
