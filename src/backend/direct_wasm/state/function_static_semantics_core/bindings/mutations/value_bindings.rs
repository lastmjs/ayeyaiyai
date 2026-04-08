use super::super::super::FunctionStaticSemanticsState;
use crate::backend::direct_wasm::{LocalFunctionBinding, ProxyValueBinding, StaticValueKind};
use crate::ir::hir::Expression;

impl FunctionStaticSemanticsState {
    pub(in crate::backend::direct_wasm) fn set_local_value_binding(
        &mut self,
        name: &str,
        value: Expression,
    ) {
        self.values.set_local_value_binding(name, value);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_value_binding(&mut self, name: &str) {
        self.values.clear_local_value_binding(name);
    }

    pub(in crate::backend::direct_wasm) fn set_local_function_binding(
        &mut self,
        name: &str,
        binding: LocalFunctionBinding,
    ) {
        self.values.set_local_function_binding(name, binding);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_function_binding(&mut self, name: &str) {
        self.values.clear_local_function_binding(name);
    }

    pub(in crate::backend::direct_wasm) fn set_local_kind(
        &mut self,
        name: &str,
        kind: StaticValueKind,
    ) {
        self.values.set_local_kind(name, kind);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_kind(&mut self, name: &str) {
        self.values.clear_local_kind(name);
    }

    pub(in crate::backend::direct_wasm) fn set_local_proxy_binding(
        &mut self,
        name: &str,
        binding: ProxyValueBinding,
    ) {
        self.values.set_local_proxy_binding(name, binding);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_proxy_binding(&mut self, name: &str) {
        self.values.clear_local_proxy_binding(name);
    }
}
