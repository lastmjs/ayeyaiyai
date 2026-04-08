use super::*;

#[path = "environment/contracts.rs"]
mod contracts;
pub(in crate::backend::direct_wasm) use contracts::*;

#[path = "environment/identifier_sources.rs"]
mod identifier_sources;
pub(in crate::backend::direct_wasm) use identifier_sources::*;

#[path = "environment/builtin_array_sources.rs"]
mod builtin_array_sources;
pub(in crate::backend::direct_wasm) use builtin_array_sources::*;

#[path = "environment/binding_maps.rs"]
mod binding_maps;
pub(in crate::backend::direct_wasm) use binding_maps::*;

#[path = "environment/concrete_impls.rs"]
mod concrete_impls;

#[path = "environment/expression_sources.rs"]
mod expression_sources;
pub(in crate::backend::direct_wasm) use expression_sources::*;
