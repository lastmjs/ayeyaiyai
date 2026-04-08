use super::super::super::super::super::*;
use crate::backend::direct_wasm::GlobalValueService;

impl GlobalValueService {
    pub(in crate::backend::direct_wasm) fn sync_proxy_binding(
        &mut self,
        name: &str,
        binding: Option<ProxyValueBinding>,
    ) {
        if let Some(binding) = binding {
            self.proxy_bindings.insert(name.to_string(), binding);
        } else {
            self.proxy_bindings.remove(name);
        }
    }

    pub(in crate::backend::direct_wasm) fn clear_proxy_binding(&mut self, name: &str) {
        self.proxy_bindings.remove(name);
    }
}
