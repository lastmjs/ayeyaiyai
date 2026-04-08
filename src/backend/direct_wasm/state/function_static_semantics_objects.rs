use crate::backend::direct_wasm::{
    LocalFunctionBinding, MemberFunctionBindingKey, ObjectValueBinding, PropertyDescriptorBinding,
};
use std::collections::{BTreeMap, HashMap};

#[path = "function_static_semantics_objects/cleanup.rs"]
mod cleanup;
#[path = "function_static_semantics_objects/member_bindings.rs"]
mod member_bindings;
#[path = "function_static_semantics_objects/object_bindings.rs"]
mod object_bindings;

#[derive(Clone, Default)]
pub(in crate::backend::direct_wasm) struct FunctionObjectSemanticsState {
    pub(in crate::backend::direct_wasm) member_function_bindings:
        HashMap<MemberFunctionBindingKey, LocalFunctionBinding>,
    pub(in crate::backend::direct_wasm) member_function_capture_slots:
        HashMap<MemberFunctionBindingKey, BTreeMap<String, String>>,
    pub(in crate::backend::direct_wasm) member_getter_bindings:
        HashMap<MemberFunctionBindingKey, LocalFunctionBinding>,
    pub(in crate::backend::direct_wasm) member_setter_bindings:
        HashMap<MemberFunctionBindingKey, LocalFunctionBinding>,
    pub(in crate::backend::direct_wasm) local_object_bindings: HashMap<String, ObjectValueBinding>,
    pub(in crate::backend::direct_wasm) local_prototype_object_bindings:
        HashMap<String, ObjectValueBinding>,
    pub(in crate::backend::direct_wasm) local_descriptor_bindings:
        HashMap<String, PropertyDescriptorBinding>,
}

impl FunctionObjectSemanticsState {
    pub(in crate::backend::direct_wasm) fn from_prepared_bindings(
        local_object_bindings: HashMap<String, ObjectValueBinding>,
    ) -> Self {
        Self {
            member_function_bindings: HashMap::new(),
            member_function_capture_slots: HashMap::new(),
            member_getter_bindings: HashMap::new(),
            member_setter_bindings: HashMap::new(),
            local_object_bindings,
            local_prototype_object_bindings: HashMap::new(),
            local_descriptor_bindings: HashMap::new(),
        }
    }
}
