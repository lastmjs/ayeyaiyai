use super::*;

impl FunctionCompilerState {
    pub(in crate::backend::direct_wasm) fn from_prepared_entry_state(
        entry_state: PreparedFunctionEntryState,
    ) -> Self {
        let PreparedFunctionEntryState {
            parameter_state,
            runtime,
            static_bindings,
            execution_context,
        } = entry_state;
        Self {
            parameters: FunctionParameterState::from_prepared_state(parameter_state),
            runtime: FunctionRuntimeState::from_prepared_state(runtime),
            speculation: FunctionSpeculationState {
                static_semantics: FunctionStaticSemanticsState::from_prepared_bindings(
                    static_bindings,
                ),
                execution_context: FunctionExecutionContextState::from_prepared_context(
                    execution_context,
                ),
            },
            emission: FunctionEmissionState::default(),
        }
    }
}
