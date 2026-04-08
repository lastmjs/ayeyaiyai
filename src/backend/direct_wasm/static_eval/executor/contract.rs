#[path = "contract/context.rs"]
mod context;
#[path = "contract/evaluation.rs"]
mod evaluation;
#[path = "contract/hooks.rs"]
mod hooks;
#[path = "contract/materialization.rs"]
mod materialization;
#[path = "contract/mutations.rs"]
mod mutations;
#[path = "contract/statements.rs"]
mod statements;

pub(in crate::backend::direct_wasm) use context::StaticExecutorContext;
pub(in crate::backend::direct_wasm) use evaluation::StaticExpressionEvaluation;
pub(in crate::backend::direct_wasm) use hooks::StaticExpressionHooks;
pub(in crate::backend::direct_wasm) use materialization::StaticExpressionMaterialization;
pub(in crate::backend::direct_wasm) use mutations::StaticBindingMutationExecutor;
pub(in crate::backend::direct_wasm) use statements::StaticStatementExecutionExecutor;

pub(in crate::backend::direct_wasm) trait StaticExpressionExecutor:
    StaticStatementExecutionExecutor
{
}

impl<T> StaticExpressionExecutor for T where T: StaticStatementExecutionExecutor + ?Sized {}
