use super::super::super::*;

pub(in crate::backend::direct_wasm) trait GlobalStaticSemanticsReadServices {
    fn names(&self) -> &GlobalNameService;
    fn values(&self) -> &GlobalValueService;
    fn functions(&self) -> &GlobalFunctionService;
    fn members(&self) -> &GlobalMemberService;
}

pub(super) fn snapshot_global_static_semantics(
    access: &impl GlobalStaticSemanticsReadServices,
) -> GlobalStaticSemanticsSnapshot {
    GlobalStaticSemanticsSnapshot {
        names: access.names().clone(),
        values: access.values().clone(),
        functions: access.functions().clone(),
        members: access.members().clone(),
    }
}

pub(in crate::backend::direct_wasm) trait GlobalStaticSemanticsWriteServices:
    GlobalStaticSemanticsReadServices
{
    fn clear_global_static_binding_metadata(&mut self, name: &str);
    fn clear_global_binding_state(&mut self, name: &str);
}

pub(super) fn clear_global_static_binding_metadata(
    access: &mut impl GlobalStaticSemanticsWriteServices,
    name: &str,
) {
    access.clear_global_static_binding_metadata(name);
}

pub(super) fn clear_global_binding_state(
    access: &mut impl GlobalStaticSemanticsWriteServices,
    name: &str,
) {
    access.clear_global_binding_state(name);
}
