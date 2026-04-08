#[path = "state_eval/arrays.rs"]
mod arrays;
#[path = "state_eval/assigned_member_policy.rs"]
mod assigned_member_policy;
#[path = "state_eval/compiler_services.rs"]
mod compiler_services;
#[path = "state_eval/context.rs"]
mod context;
#[path = "state_eval/expression_environment.rs"]
mod expression_environment;
#[path = "state_eval/identifiers.rs"]
mod identifiers;
#[path = "state_eval/materialization_policy.rs"]
mod materialization_policy;
#[path = "state_eval/special_expressions.rs"]
mod special_expressions;
#[path = "state_eval/user_functions.rs"]
mod user_functions;

pub(in crate::backend::direct_wasm) use context::FunctionStaticEvalContext;
