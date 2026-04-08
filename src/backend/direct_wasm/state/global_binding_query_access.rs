use crate::backend::direct_wasm::{ImplicitGlobalBinding, LocalFunctionBinding, StaticValueKind};

pub(in crate::backend::direct_wasm) trait GlobalBindingIndexQueryAccess {
    fn resolve_global_binding_index(&self, name: &str) -> Option<u32>;
    fn global_binding_index(&self, name: &str) -> Option<u32>;
    fn global_binding_count(&self) -> u32;
}

pub(in crate::backend::direct_wasm) trait GlobalBindingPresenceQueryAccess {
    fn global_has_binding(&self, name: &str) -> bool;
    fn global_has_lexical_binding(&self, name: &str) -> bool;
    fn global_has_implicit_binding(&self, name: &str) -> bool;
}

pub(in crate::backend::direct_wasm) trait GlobalImplicitBindingQueryAccess {
    fn implicit_global_binding(&self, name: &str) -> Option<ImplicitGlobalBinding>;
    fn implicit_global_binding_count(&self) -> u32;
}

pub(in crate::backend::direct_wasm) trait GlobalBindingKindQueryAccess {
    fn global_binding_kind(&self, name: &str) -> Option<StaticValueKind>;
}

pub(in crate::backend::direct_wasm) trait GlobalFunctionBindingQueryAccess {
    fn find_global_user_function_binding_name(&self, function_name: &str) -> Option<String>;
    fn global_function_binding(&self, name: &str) -> Option<&LocalFunctionBinding>;
}
