use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn with_isolated_indirect_eval_state<T>(
        &mut self,
        callback: impl FnOnce(&mut Self) -> DirectResult<T>,
    ) -> DirectResult<T> {
        let transaction = IsolatedIndirectEvalTransaction::capture_and_enter(&mut self.state);

        let result = callback(self);

        transaction.restore(&mut self.state);

        result
    }
}
