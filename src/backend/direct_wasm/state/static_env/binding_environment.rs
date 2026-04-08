use super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct GlobalBindingEnvironment {
    pub(in crate::backend::direct_wasm) value_bindings: HashMap<String, Expression>,
    pub(in crate::backend::direct_wasm) object_bindings: HashMap<String, ObjectValueBinding>,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct SharedGlobalBindingEnvironment {
    pub(in crate::backend::direct_wasm) value_bindings: Arc<HashMap<String, Expression>>,
    pub(in crate::backend::direct_wasm) object_bindings: Arc<HashMap<String, ObjectValueBinding>>,
}

impl SharedGlobalBindingEnvironment {
    pub(in crate::backend::direct_wasm) fn from_binding_environment(
        environment: &GlobalBindingEnvironment,
    ) -> Self {
        Self {
            value_bindings: Arc::new(environment.value_bindings.clone()),
            object_bindings: Arc::new(environment.object_bindings.clone()),
        }
    }
}
