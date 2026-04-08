use super::super::*;

impl FunctionCompilerState {
    pub(in crate::backend::direct_wasm) fn enter_isolated_indirect_eval_state(&mut self) {
        self.runtime.clear_isolated_indirect_eval_state();
        self.parameters.clear_isolated_indirect_eval_state();
        self.speculation
            .static_semantics
            .clear_isolated_indirect_eval_state();
        self.emission
            .lexical_scopes
            .clear_isolated_indirect_eval_state();
        self.speculation
            .execution_context
            .reset_isolated_indirect_eval_entry();
    }
}
