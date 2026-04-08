use super::super::super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct StaticResolutionEnvironment {
    pub(in crate::backend::direct_wasm) local_bindings: HashMap<String, Expression>,
    pub(in crate::backend::direct_wasm) global_value_bindings: Arc<HashMap<String, Expression>>,
    pub(in crate::backend::direct_wasm) global_value_overrides: HashMap<String, Expression>,
    pub(in crate::backend::direct_wasm) local_object_bindings: HashMap<String, ObjectValueBinding>,
    pub(in crate::backend::direct_wasm) global_object_bindings:
        Arc<HashMap<String, ObjectValueBinding>>,
    pub(in crate::backend::direct_wasm) global_object_overrides:
        HashMap<String, Option<ObjectValueBinding>>,
    pub(in crate::backend::direct_wasm) local_descriptor_bindings:
        HashMap<String, PropertyDescriptorBinding>,
}
