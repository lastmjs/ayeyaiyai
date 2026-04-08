use super::super::super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct FunctionExecutionContextSnapshot {
    pub(in crate::backend::direct_wasm) execution_context: FunctionExecutionContextState,
}

impl FunctionExecutionContextState {
    pub(in crate::backend::direct_wasm) fn snapshot(&self) -> FunctionExecutionContextSnapshot {
        FunctionExecutionContextSnapshot {
            execution_context: self.clone(),
        }
    }

    pub(in crate::backend::direct_wasm) fn restore(
        &mut self,
        snapshot: FunctionExecutionContextSnapshot,
    ) {
        *self = snapshot.execution_context;
    }
}

pub(in crate::backend::direct_wasm) struct UserFunctionExecutionContextSnapshot {
    pub(in crate::backend::direct_wasm) execution_context: FunctionExecutionContextState,
}
