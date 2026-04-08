use super::*;

#[path = "shared/global_context.rs"]
mod global_context;
pub(in crate::backend::direct_wasm) use global_context::*;

#[path = "shared/user_functions.rs"]
mod user_functions;
pub(in crate::backend::direct_wasm) use user_functions::*;
