use super::super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct GlobalStaticSemanticsSnapshot {
    pub(in crate::backend::direct_wasm) names: GlobalNameService,
    pub(in crate::backend::direct_wasm) values: GlobalValueService,
    pub(in crate::backend::direct_wasm) functions: GlobalFunctionService,
    pub(in crate::backend::direct_wasm) members: GlobalMemberService,
}

pub(in crate::backend::direct_wasm) struct GlobalStaticSemanticsTransaction {
    pub(in crate::backend::direct_wasm) snapshot: GlobalStaticSemanticsSnapshot,
}
