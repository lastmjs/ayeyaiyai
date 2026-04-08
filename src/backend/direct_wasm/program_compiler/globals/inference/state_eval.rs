#[path = "state_eval/arrays.rs"]
mod arrays;
#[path = "state_eval/compiler_services.rs"]
mod compiler_services;
#[path = "state_eval/context.rs"]
mod context;
#[path = "state_eval/expression_policies.rs"]
mod expression_policies;
#[path = "state_eval/identifiers.rs"]
mod identifiers;
#[path = "state_eval/user_functions.rs"]
mod user_functions;

pub(in crate::backend::direct_wasm) use context::ProgramStaticEvalContext;
