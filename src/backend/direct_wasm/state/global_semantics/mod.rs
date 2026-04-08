use super::*;

mod access;
mod functions;
mod members;
mod names;
mod value_bindings;
mod value_queries;

pub(in crate::backend::direct_wasm) use access::GlobalSemanticState;
pub(in crate::backend::direct_wasm) use functions::GlobalFunctionService;
pub(in crate::backend::direct_wasm) use members::GlobalMemberService;
pub(in crate::backend::direct_wasm) use names::GlobalNameService;
pub(in crate::backend::direct_wasm) use value_bindings::{
    GlobalObjectRuntimePrototypeBinding, GlobalValueService,
};
