use super::super::super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct GlobalStaticEvaluationEnvironment {
    pub(in crate::backend::direct_wasm) local_bindings: HashMap<String, Expression>,
    pub(in crate::backend::direct_wasm) value_bindings: HashMap<String, Expression>,
    pub(in crate::backend::direct_wasm) object_bindings: HashMap<String, ObjectValueBinding>,
}
