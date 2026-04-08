use super::FunctionValueSemanticsState;
use crate::backend::direct_wasm::StaticValueKind;
use crate::ir::hir::Expression;
use std::collections::HashMap;

impl FunctionValueSemanticsState {
    pub(in crate::backend::direct_wasm) fn local_value_binding(
        &self,
        name: &str,
    ) -> Option<&Expression> {
        self.local_value_bindings.get(name)
    }

    pub(in crate::backend::direct_wasm) fn local_value_bindings_snapshot(
        &self,
    ) -> HashMap<String, Expression> {
        self.local_value_bindings.clone()
    }

    pub(in crate::backend::direct_wasm) fn set_local_value_binding(
        &mut self,
        name: &str,
        value: Expression,
    ) {
        self.local_value_bindings.insert(name.to_string(), value);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_value_binding(&mut self, name: &str) {
        self.local_value_bindings.remove(name);
    }

    pub(in crate::backend::direct_wasm) fn local_kind(
        &self,
        name: &str,
    ) -> Option<StaticValueKind> {
        self.local_kinds.get(name).copied()
    }

    pub(in crate::backend::direct_wasm) fn set_local_kind(
        &mut self,
        name: &str,
        kind: StaticValueKind,
    ) {
        self.local_kinds.insert(name.to_string(), kind);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_kind(&mut self, name: &str) {
        self.local_kinds.remove(name);
    }
}
