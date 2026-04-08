use super::super::{FunctionCompilerBackend, ImplicitGlobalBinding};

impl<'a> FunctionCompilerBackend<'a> {
    pub(in crate::backend::direct_wasm) fn ensure_implicit_global_binding(
        &mut self,
        name: &str,
    ) -> ImplicitGlobalBinding {
        self.global_semantics.ensure_implicit_binding(name)
    }
}
