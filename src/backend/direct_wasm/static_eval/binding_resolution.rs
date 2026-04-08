use super::*;

#[path = "binding_resolution/object_bindings.rs"]
mod object_bindings;
pub(in crate::backend::direct_wasm) use object_bindings::*;

#[path = "binding_resolution/identifiers.rs"]
mod identifiers;
pub(in crate::backend::direct_wasm) use identifiers::*;

#[path = "binding_resolution/members.rs"]
mod members;
pub(in crate::backend::direct_wasm) use members::*;

#[path = "binding_resolution/stateful_materialization.rs"]
mod stateful_materialization;
pub(in crate::backend::direct_wasm) use stateful_materialization::*;
