use super::super::*;

impl CompilerState {
    pub(in crate::backend::direct_wasm) fn ensure_global_binding_index(
        &mut self,
        name: &str,
        next_global_index: &mut u32,
    ) {
        self.global_semantics
            .ensure_global_binding_index(name, next_global_index);
    }

    pub(in crate::backend::direct_wasm) fn mark_global_lexical_binding(&mut self, name: &str) {
        self.global_semantics.mark_global_lexical_binding(name);
    }

    pub(in crate::backend::direct_wasm) fn clear_global_binding_state(&mut self, name: &str) {
        self.global_semantics.clear_global_binding_state(name);
    }

    pub(in crate::backend::direct_wasm) fn set_global_binding_kind(
        &mut self,
        name: &str,
        kind: StaticValueKind,
    ) {
        self.global_semantics.set_global_binding_kind(name, kind);
    }
}
