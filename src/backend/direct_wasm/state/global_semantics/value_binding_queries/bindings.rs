use super::super::super::super::*;
use crate::backend::direct_wasm::GlobalValueService;

impl GlobalValueService {
    pub(in crate::backend::direct_wasm) fn value_binding(&self, name: &str) -> Option<&Expression> {
        self.value_bindings.get(name)
    }

    pub(in crate::backend::direct_wasm) fn object_binding(
        &self,
        name: &str,
    ) -> Option<&ObjectValueBinding> {
        self.object_bindings.get(name)
    }

    pub(in crate::backend::direct_wasm) fn object_binding_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut ObjectValueBinding> {
        self.object_bindings.get_mut(name)
    }

    pub(in crate::backend::direct_wasm) fn array_binding(
        &self,
        name: &str,
    ) -> Option<&ArrayValueBinding> {
        self.array_bindings.get(name)
    }

    pub(in crate::backend::direct_wasm) fn array_bindings(
        &self,
    ) -> &HashMap<String, ArrayValueBinding> {
        &self.array_bindings
    }

    pub(in crate::backend::direct_wasm) fn array_binding_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut ArrayValueBinding> {
        self.array_bindings.get_mut(name)
    }

    pub(in crate::backend::direct_wasm) fn arguments_binding(
        &self,
        name: &str,
    ) -> Option<&ArgumentsValueBinding> {
        self.arguments_bindings.get(name)
    }

    pub(in crate::backend::direct_wasm) fn proxy_binding(
        &self,
        name: &str,
    ) -> Option<&ProxyValueBinding> {
        self.proxy_bindings.get(name)
    }
}
