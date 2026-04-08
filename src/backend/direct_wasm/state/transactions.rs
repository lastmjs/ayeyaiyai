#[path = "transactions/eval.rs"]
mod eval;
#[path = "transactions/function.rs"]
mod function;
#[path = "transactions/global.rs"]
mod global;

pub(in crate::backend::direct_wasm) use eval::{
    IsolatedIndirectEvalTransaction, StaticBindingMetadataTransaction,
};
pub(in crate::backend::direct_wasm) use function::{
    FunctionStaticBindingMetadataSnapshot, FunctionStaticBindingMetadataTransaction,
    LocalStaticBindingSnapshot, LocalStaticBindingState, UserFunctionExecutionContextSnapshot,
};
pub(in crate::backend::direct_wasm) use global::{
    GlobalStaticSemanticsSnapshot, GlobalStaticSemanticsTransaction,
};
