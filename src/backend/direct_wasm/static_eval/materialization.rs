use super::*;

#[path = "materialization/binding_maps.rs"]
mod binding_maps;
pub(in crate::backend::direct_wasm) use binding_maps::*;

#[path = "materialization/structural.rs"]
mod structural;
pub(in crate::backend::direct_wasm) use structural::*;

#[path = "materialization/object_bindings.rs"]
mod object_bindings;
pub(in crate::backend::direct_wasm) use object_bindings::*;
