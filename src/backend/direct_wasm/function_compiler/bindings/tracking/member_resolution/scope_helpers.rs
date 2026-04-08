use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn with_suspended_with_scopes<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> DirectResult<T>,
    ) -> DirectResult<T> {
        let previous_with_scopes = self.state.take_with_scopes();
        let result = f(self);
        self.state.restore_with_scopes(previous_with_scopes);
        result
    }
}
