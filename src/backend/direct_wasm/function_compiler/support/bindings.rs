use super::*;

#[path = "bindings/constructor_locals.rs"]
mod constructor_locals;
#[path = "bindings/declaration_collection.rs"]
mod declaration_collection;
#[path = "bindings/delete_semantics.rs"]
mod delete_semantics;
#[path = "bindings/eval_bindings.rs"]
mod eval_bindings;
#[path = "bindings/loop_assignments.rs"]
mod loop_assignments;

pub(in crate::backend::direct_wasm) use self::{
    constructor_locals::*, declaration_collection::*, delete_semantics::*, eval_bindings::*,
    loop_assignments::*,
};
