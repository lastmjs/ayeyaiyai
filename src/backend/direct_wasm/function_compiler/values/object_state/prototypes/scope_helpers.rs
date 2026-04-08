use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn with_scope_blocks_static_identifier_resolution(
        &self,
        name: &str,
    ) -> bool {
        !self.state.emission.lexical_scopes.with_scopes.is_empty() && !name.starts_with("__ayy_")
    }
}
