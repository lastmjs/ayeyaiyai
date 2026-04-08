use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn is_identifier_bound(&self, name: &str) -> bool {
        self.lookup_identifier_kind(name).is_some()
    }

    pub(in crate::backend::direct_wasm) fn is_unshadowed_builtin_identifier(
        &self,
        name: &str,
    ) -> bool {
        self.resolve_current_local_binding(name).is_none()
            && self.backend.global_binding_index(name).is_none()
            && self.backend.global_function_binding(name).is_none()
            && !is_internal_user_function_identifier(name)
    }
}
