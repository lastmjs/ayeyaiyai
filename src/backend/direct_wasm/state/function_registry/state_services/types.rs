use super::super::*;

impl FunctionRegistryState {
    pub(in crate::backend::direct_wasm) fn reset_for_program(&mut self) {
        self.types.reset_for_program();
        self.catalog.reset_for_program();
        self.analysis.reset_for_program();
    }

    pub(in crate::backend::direct_wasm) fn user_type_index_for_arity(&mut self, arity: u32) -> u32 {
        self.types.type_index_for_arity(arity)
    }

    pub(in crate::backend::direct_wasm) fn next_user_function_index(&self) -> u32 {
        USER_FUNCTION_BASE_INDEX + self.catalog.user_functions.len() as u32
    }
}
