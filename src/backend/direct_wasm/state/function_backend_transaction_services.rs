use crate::backend::direct_wasm::{
    FunctionCompilerBackend, GlobalStaticSemanticsSnapshot, GlobalStaticSemanticsTransaction,
};

impl<'a> FunctionCompilerBackend<'a> {
    pub(in crate::backend::direct_wasm) fn snapshot_global_static_semantics(
        &self,
    ) -> GlobalStaticSemanticsSnapshot {
        self.global_semantics.snapshot()
    }

    pub(in crate::backend::direct_wasm) fn begin_global_static_semantics_transaction(
        &self,
    ) -> GlobalStaticSemanticsTransaction {
        GlobalStaticSemanticsTransaction {
            snapshot: self.snapshot_global_static_semantics(),
        }
    }

    pub(in crate::backend::direct_wasm) fn restore_global_static_semantics(
        &mut self,
        snapshot: GlobalStaticSemanticsSnapshot,
    ) {
        self.global_semantics = snapshot;
    }

    pub(in crate::backend::direct_wasm) fn restore_global_static_semantics_transaction(
        &mut self,
        transaction: GlobalStaticSemanticsTransaction,
    ) {
        self.restore_global_static_semantics(transaction.snapshot);
    }

    pub(in crate::backend::direct_wasm) fn clear_global_binding_kind(&mut self, name: &str) {
        self.global_semantics.clear_global_binding_kind(name);
    }

    pub(in crate::backend::direct_wasm) fn clear_global_binding_state(&mut self, name: &str) {
        self.global_semantics.clear_global_binding_state(name);
    }

    pub(in crate::backend::direct_wasm) fn clear_global_static_binding_metadata(
        &mut self,
        name: &str,
    ) {
        self.global_semantics
            .clear_global_static_binding_metadata(name);
    }

    pub(in crate::backend::direct_wasm) fn clear_global_object_literal_member_bindings_for_name(
        &mut self,
        name: &str,
    ) {
        self.global_semantics
            .clear_global_object_literal_member_bindings_for_name(name);
    }
}
