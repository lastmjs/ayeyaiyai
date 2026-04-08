use super::super::super::FunctionStaticSemanticsState;
use crate::backend::direct_wasm::{LocalFunctionBinding, ProxyValueBinding, StaticValueKind};
use crate::ir::hir::Expression;

impl FunctionStaticSemanticsState {
    pub(in crate::backend::direct_wasm) fn local_value_binding(
        &self,
        name: &str,
    ) -> Option<&Expression> {
        self.values.local_value_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn local_kind(
        &self,
        name: &str,
    ) -> Option<StaticValueKind> {
        self.values.local_kind(name)
    }

    pub(in crate::backend::direct_wasm) fn local_function_binding(
        &self,
        name: &str,
    ) -> Option<&LocalFunctionBinding> {
        self.values.local_function_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn has_local_function_binding(&self, name: &str) -> bool {
        self.local_function_binding(name).is_some()
    }

    pub(in crate::backend::direct_wasm) fn local_proxy_binding(
        &self,
        name: &str,
    ) -> Option<&ProxyValueBinding> {
        self.values.local_proxy_binding(name)
    }
}
