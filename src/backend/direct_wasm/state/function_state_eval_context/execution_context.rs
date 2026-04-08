use super::super::*;

impl FunctionCompilerState {
    pub(in crate::backend::direct_wasm) fn snapshot_user_function_execution_context(
        &self,
    ) -> UserFunctionExecutionContextSnapshot {
        UserFunctionExecutionContextSnapshot {
            execution_context: self.speculation.execution_context.clone(),
        }
    }

    pub(in crate::backend::direct_wasm) fn enter_user_function_execution_context(
        &mut self,
        user_function: &UserFunction,
    ) {
        self.speculation.execution_context.strict_mode = user_function.strict;
        self.speculation
            .execution_context
            .current_user_function_name = Some(user_function.name.clone());
    }

    pub(in crate::backend::direct_wasm) fn restore_user_function_execution_context(
        &mut self,
        snapshot: UserFunctionExecutionContextSnapshot,
    ) {
        self.speculation.execution_context = snapshot.execution_context;
    }
}
