use super::*;

#[path = "static_eval/environment.rs"]
mod environment;
pub(in crate::backend::direct_wasm) use environment::*;

#[path = "static_eval/binding_resolution.rs"]
mod binding_resolution;
pub(in crate::backend::direct_wasm) use binding_resolution::*;

#[path = "static_eval/object_array_helpers.rs"]
mod object_array_helpers;
pub(in crate::backend::direct_wasm) use object_array_helpers::*;

#[path = "static_eval/materialization.rs"]
mod materialization;
pub(in crate::backend::direct_wasm) use materialization::*;

#[path = "static_eval/execution.rs"]
mod execution;
pub(in crate::backend::direct_wasm) use execution::{
    StaticUserFunctionBindingExecutor, StaticUserFunctionBindingSource,
    execute_static_user_function_binding_in_environment,
};

#[path = "static_eval/shared_expression_eval.rs"]
mod shared_expression_eval;
pub(in crate::backend::direct_wasm) use shared_expression_eval::*;

#[path = "static_eval/executor.rs"]
mod executor;
pub(in crate::backend::direct_wasm) use executor::*;
