use super::super::super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct FunctionLexicalScopeSnapshot {
    pub(in crate::backend::direct_wasm) lexical_scopes: FunctionLexicalScopeState,
}

impl FunctionLexicalScopeState {
    pub(in crate::backend::direct_wasm) fn snapshot(&self) -> FunctionLexicalScopeSnapshot {
        FunctionLexicalScopeSnapshot {
            lexical_scopes: self.clone(),
        }
    }

    pub(in crate::backend::direct_wasm) fn restore(
        &mut self,
        snapshot: FunctionLexicalScopeSnapshot,
    ) {
        *self = snapshot.lexical_scopes;
    }
}
