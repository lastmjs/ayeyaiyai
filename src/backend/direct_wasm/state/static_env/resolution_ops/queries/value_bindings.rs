use super::super::super::super::*;
use super::super::super::StaticResolutionEnvironment;

impl StaticResolutionEnvironment {
    pub(in crate::backend::direct_wasm) fn binding(&self, name: &str) -> Option<&Expression> {
        self.local_bindings
            .get(name)
            .or_else(|| self.global_value_overrides.get(name))
            .or_else(|| self.global_value_bindings.get(name))
    }

    pub(in crate::backend::direct_wasm) fn local_binding(&self, name: &str) -> Option<&Expression> {
        self.local_bindings.get(name)
    }

    pub(in crate::backend::direct_wasm) fn global_value_binding(
        &self,
        name: &str,
    ) -> Option<&Expression> {
        self.global_value_overrides
            .get(name)
            .or_else(|| self.global_value_bindings.get(name))
    }
}
