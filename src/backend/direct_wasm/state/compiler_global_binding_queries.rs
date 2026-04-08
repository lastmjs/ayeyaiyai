use super::{
    CompilerState, GlobalBindingIndexQueryAccess, GlobalBindingKindQueryAccess,
    GlobalBindingPresenceQueryAccess, GlobalFunctionBindingQueryAccess,
    GlobalImplicitBindingQueryAccess, GlobalStaticSemanticsSnapshot, ImplicitGlobalBinding,
    LocalFunctionBinding, StaticValueKind,
};

impl GlobalBindingIndexQueryAccess for CompilerState {
    fn resolve_global_binding_index(&self, name: &str) -> Option<u32> {
        self.global_semantics
            .global_names()
            .resolve_binding_index(name)
    }

    fn global_binding_index(&self, name: &str) -> Option<u32> {
        self.global_semantics.global_names().binding_index(name)
    }

    fn global_binding_count(&self) -> u32 {
        self.global_semantics.global_names().binding_count()
    }
}

impl GlobalBindingPresenceQueryAccess for CompilerState {
    fn global_has_binding(&self, name: &str) -> bool {
        self.global_semantics.global_names().has_binding(name)
    }

    fn global_has_lexical_binding(&self, name: &str) -> bool {
        self.global_semantics
            .global_names()
            .has_lexical_binding(name)
    }

    fn global_has_implicit_binding(&self, name: &str) -> bool {
        self.global_semantics
            .global_names()
            .has_implicit_binding(name)
    }
}

impl GlobalImplicitBindingQueryAccess for CompilerState {
    fn implicit_global_binding(&self, name: &str) -> Option<ImplicitGlobalBinding> {
        self.global_semantics.global_names().implicit_binding(name)
    }

    fn implicit_global_binding_count(&self) -> u32 {
        self.global_semantics
            .global_names()
            .implicit_binding_count()
    }
}

impl GlobalBindingKindQueryAccess for CompilerState {
    fn global_binding_kind(&self, name: &str) -> Option<StaticValueKind> {
        self.global_semantics.global_names().kind(name)
    }
}

impl GlobalFunctionBindingQueryAccess for CompilerState {
    fn find_global_user_function_binding_name(&self, function_name: &str) -> Option<String> {
        self.global_semantics
            .global_functions()
            .find_user_function_binding_name(function_name)
    }

    fn global_function_binding(&self, name: &str) -> Option<&LocalFunctionBinding> {
        self.global_semantics
            .global_functions()
            .function_binding(name)
    }
}

impl CompilerState {
    pub(in crate::backend::direct_wasm) fn snapshot_global_static_semantics(
        &self,
    ) -> GlobalStaticSemanticsSnapshot {
        self.global_semantics.snapshot()
    }
}
