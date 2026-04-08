use super::*;

#[path = "execution/statement_execution.rs"]
mod statement_execution;
pub(in crate::backend::direct_wasm) use statement_execution::*;

#[path = "execution/user_functions.rs"]
mod user_functions;
pub(in crate::backend::direct_wasm) use user_functions::*;
