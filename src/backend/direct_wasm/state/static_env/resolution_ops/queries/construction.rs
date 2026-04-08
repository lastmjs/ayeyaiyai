use super::super::super::super::*;
use super::super::super::StaticResolutionEnvironment;

impl StaticResolutionEnvironment {
    pub(in crate::backend::direct_wasm) fn from_binding_snapshots(
        global_value_bindings: Arc<HashMap<String, Expression>>,
        global_object_bindings: Arc<HashMap<String, ObjectValueBinding>>,
        local_bindings: HashMap<String, Expression>,
        local_object_bindings: HashMap<String, ObjectValueBinding>,
        local_descriptor_bindings: HashMap<String, PropertyDescriptorBinding>,
    ) -> Self {
        Self {
            local_bindings,
            global_value_bindings,
            global_value_overrides: HashMap::new(),
            local_object_bindings,
            global_object_bindings,
            global_object_overrides: HashMap::new(),
            local_descriptor_bindings,
        }
    }

    pub(in crate::backend::direct_wasm) fn fork(&self) -> Self {
        self.clone()
    }

    pub(in crate::backend::direct_wasm) fn into_local_bindings(
        self,
    ) -> HashMap<String, Expression> {
        self.local_bindings
    }
}
