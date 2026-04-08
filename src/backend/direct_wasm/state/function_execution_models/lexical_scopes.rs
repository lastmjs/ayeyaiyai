use super::*;

#[derive(Clone, Default)]
pub(in crate::backend::direct_wasm) struct FunctionLexicalScopeState {
    pub(in crate::backend::direct_wasm) active_eval_lexical_scopes:
        Vec<Vec<(String, Option<String>)>>,
    pub(in crate::backend::direct_wasm) active_eval_lexical_binding_counts: HashMap<String, u32>,
    pub(in crate::backend::direct_wasm) active_scoped_lexical_bindings:
        HashMap<String, Vec<String>>,
    pub(in crate::backend::direct_wasm) with_scopes: Vec<Expression>,
}

impl FunctionLexicalScopeState {
    pub(in crate::backend::direct_wasm) fn clear_isolated_indirect_eval_state(&mut self) {
        *self = Self::default();
    }
}
