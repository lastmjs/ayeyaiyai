#[path = "statement_execution/block.rs"]
mod block;
#[path = "statement_execution/executor.rs"]
mod executor;
#[path = "statement_execution/try_execution.rs"]
mod try_execution;

pub(in crate::backend::direct_wasm) use self::{
    block::{execute_static_statement_block, execute_static_statement_value},
    executor::StaticStatementExecutor,
    try_execution::execute_static_try_statement,
};
