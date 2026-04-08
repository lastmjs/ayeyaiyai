use super::*;

#[path = "shared_expression_eval/binary_ops.rs"]
mod binary_ops;
pub(in crate::backend::direct_wasm) use binary_ops::*;

#[path = "shared_expression_eval/expression_execution.rs"]
mod expression_execution;
pub(in crate::backend::direct_wasm) use expression_execution::*;

#[path = "shared_expression_eval/binding_sync.rs"]
mod binding_sync;
pub(in crate::backend::direct_wasm) use binding_sync::*;
