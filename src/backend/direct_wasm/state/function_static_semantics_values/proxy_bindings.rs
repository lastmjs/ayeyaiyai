use super::FunctionValueSemanticsState;
use crate::backend::direct_wasm::ProxyValueBinding;

impl FunctionValueSemanticsState {
    pub(in crate::backend::direct_wasm) fn local_proxy_binding(
        &self,
        name: &str,
    ) -> Option<&ProxyValueBinding> {
        self.local_proxy_bindings.get(name)
    }

    pub(in crate::backend::direct_wasm) fn set_local_proxy_binding(
        &mut self,
        name: &str,
        binding: ProxyValueBinding,
    ) {
        self.local_proxy_bindings.insert(name.to_string(), binding);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_proxy_binding(&mut self, name: &str) {
        self.local_proxy_bindings.remove(name);
    }
}
