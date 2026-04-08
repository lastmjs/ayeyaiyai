#[path = "global_evaluation/binding_ops.rs"]
mod binding_ops;
#[path = "global_evaluation/transactions.rs"]
mod transactions;
#[path = "global_evaluation_types.rs"]
mod types;

pub(in crate::backend::direct_wasm) use types::GlobalStaticEvaluationEnvironment;
