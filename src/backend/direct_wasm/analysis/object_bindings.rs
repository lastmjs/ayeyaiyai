use super::*;

#[path = "object_bindings/mutations.rs"]
mod mutations;
#[path = "object_bindings/property_views.rs"]
mod property_views;
#[path = "object_bindings/static_expansion.rs"]
mod static_expansion;
#[path = "object_bindings/substitution.rs"]
mod substitution;

pub(in crate::backend::direct_wasm) use mutations::*;
pub(in crate::backend::direct_wasm) use property_views::*;
pub(in crate::backend::direct_wasm) use static_expansion::*;
pub(in crate::backend::direct_wasm) use substitution::*;
