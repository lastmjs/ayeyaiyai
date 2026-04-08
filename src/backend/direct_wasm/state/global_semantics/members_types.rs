use super::super::*;

#[derive(Clone, Default)]
pub(in crate::backend::direct_wasm) struct GlobalMemberService {
    pub(in crate::backend::direct_wasm) member_function_bindings:
        HashMap<MemberFunctionBindingKey, LocalFunctionBinding>,
    pub(in crate::backend::direct_wasm) member_function_capture_slots:
        HashMap<MemberFunctionBindingKey, BTreeMap<String, String>>,
    pub(in crate::backend::direct_wasm) member_getter_bindings:
        HashMap<MemberFunctionBindingKey, LocalFunctionBinding>,
    pub(in crate::backend::direct_wasm) member_setter_bindings:
        HashMap<MemberFunctionBindingKey, LocalFunctionBinding>,
}
