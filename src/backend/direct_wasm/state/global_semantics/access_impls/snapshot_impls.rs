use super::super::super::super::*;
use super::super::access_trait::{
    clear_global_binding_state, clear_global_static_binding_metadata,
    snapshot_global_static_semantics,
};

impl GlobalStaticSemanticsSnapshot {
    pub(in crate::backend::direct_wasm) fn global_names(&self) -> &GlobalNameService {
        &self.names
    }

    pub(in crate::backend::direct_wasm) fn global_functions(&self) -> &GlobalFunctionService {
        &self.functions
    }

    pub(in crate::backend::direct_wasm) fn global_members(&self) -> &GlobalMemberService {
        &self.members
    }

    pub(in crate::backend::direct_wasm) fn global_members_mut(
        &mut self,
    ) -> &mut GlobalMemberService {
        &mut self.members
    }

    pub(in crate::backend::direct_wasm) fn snapshot(&self) -> GlobalStaticSemanticsSnapshot {
        snapshot_global_static_semantics(self)
    }

    pub(in crate::backend::direct_wasm) fn set_global_binding_kind(
        &mut self,
        name: &str,
        kind: StaticValueKind,
    ) {
        self.names.set_kind(name, kind);
    }

    pub(in crate::backend::direct_wasm) fn ensure_implicit_binding(
        &mut self,
        name: &str,
    ) -> ImplicitGlobalBinding {
        self.names.ensure_implicit_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn set_global_function_binding(
        &mut self,
        name: &str,
        binding: LocalFunctionBinding,
    ) {
        self.functions.set_function_binding(name, binding);
    }

    pub(in crate::backend::direct_wasm) fn clear_global_function_binding(&mut self, name: &str) {
        self.functions.clear_function_binding(name);
    }

    pub(in crate::backend::direct_wasm) fn clear_global_binding_kind(&mut self, name: &str) {
        self.names.clear_kind(name);
    }

    pub(in crate::backend::direct_wasm) fn clear_global_static_binding_metadata(
        &mut self,
        name: &str,
    ) {
        clear_global_static_binding_metadata(self, name);
    }

    pub(in crate::backend::direct_wasm) fn clear_global_binding_state(&mut self, name: &str) {
        clear_global_binding_state(self, name);
    }

    pub(in crate::backend::direct_wasm) fn clear_global_object_literal_member_bindings_for_name(
        &mut self,
        name: &str,
    ) {
        self.members.clear_bindings_for_name(name, false);
    }
}
