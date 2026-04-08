use super::super::super::*;

impl FunctionCompilerState {
    pub(in crate::backend::direct_wasm) fn take_with_scopes(&mut self) -> Vec<Expression> {
        std::mem::take(&mut self.emission.lexical_scopes.with_scopes)
    }

    pub(in crate::backend::direct_wasm) fn restore_with_scopes(
        &mut self,
        with_scopes: Vec<Expression>,
    ) {
        self.emission.lexical_scopes.with_scopes = with_scopes;
    }

    pub(in crate::backend::direct_wasm) fn push_with_scope(&mut self, with_scope: Expression) {
        self.emission.lexical_scopes.with_scopes.push(with_scope);
    }

    pub(in crate::backend::direct_wasm) fn pop_with_scope(&mut self) {
        self.emission.lexical_scopes.with_scopes.pop();
    }
}
