use super::super::super::*;

#[derive(Clone, Default)]
pub(in crate::backend::direct_wasm) struct GlobalSemanticState {
    pub(in crate::backend::direct_wasm) names: GlobalNameService,
    pub(in crate::backend::direct_wasm) values: GlobalValueService,
    pub(in crate::backend::direct_wasm) functions: GlobalFunctionService,
    pub(in crate::backend::direct_wasm) members: GlobalMemberService,
}

impl GlobalSemanticState {
    pub(in crate::backend::direct_wasm) fn reset_for_program(&mut self) {
        self.names.reset_for_program();
        self.values.reset_for_program();
        self.functions.reset_for_program();
        self.members.reset_for_program();
    }
}
