use super::super::super::*;
use super::super::function::{
    FunctionExecutionContextSnapshot, FunctionLexicalScopeSnapshot,
    FunctionParameterIsolatedIndirectEvalSnapshot, FunctionRuntimeIsolatedIndirectEvalSnapshot,
    FunctionStaticBindingMetadataTransaction,
};

pub(in crate::backend::direct_wasm) struct IsolatedIndirectEvalTransaction {
    pub(in crate::backend::direct_wasm) runtime_state: FunctionRuntimeIsolatedIndirectEvalSnapshot,
    pub(in crate::backend::direct_wasm) parameter_state:
        FunctionParameterIsolatedIndirectEvalSnapshot,
    pub(in crate::backend::direct_wasm) static_binding_state:
        FunctionStaticBindingMetadataTransaction,
    pub(in crate::backend::direct_wasm) lexical_scope_state: FunctionLexicalScopeSnapshot,
    pub(in crate::backend::direct_wasm) execution_context_state: FunctionExecutionContextSnapshot,
}

impl IsolatedIndirectEvalTransaction {
    pub(in crate::backend::direct_wasm) fn capture_and_enter(
        state: &mut FunctionCompilerState,
    ) -> IsolatedIndirectEvalTransaction {
        let transaction = IsolatedIndirectEvalTransaction {
            runtime_state: state.runtime.snapshot_isolated_indirect_eval(),
            parameter_state: state.parameters.capture_isolated_indirect_eval(),
            static_binding_state: FunctionStaticBindingMetadataTransaction::capture(state),
            lexical_scope_state: state.emission.lexical_scopes.snapshot(),
            execution_context_state: state.speculation.execution_context.snapshot(),
        };
        state.enter_isolated_indirect_eval_state();
        transaction
    }

    pub(in crate::backend::direct_wasm) fn restore(self, state: &mut FunctionCompilerState) {
        state
            .runtime
            .restore_isolated_indirect_eval(self.runtime_state);
        state
            .parameters
            .restore_isolated_indirect_eval(self.parameter_state);
        self.static_binding_state.restore(state);
        state
            .emission
            .lexical_scopes
            .restore(self.lexical_scope_state);
        state
            .speculation
            .execution_context
            .restore(self.execution_context_state);
    }
}
