use super::*;

#[path = "returned_member_values/collection.rs"]
mod collection;
#[path = "returned_member_values/expression_traversal.rs"]
mod expression_traversal;
#[path = "returned_member_values/resolution.rs"]
mod resolution;
#[path = "returned_member_values/statement_traversal.rs"]
mod statement_traversal;

pub(in crate::backend::direct_wasm) use collection::*;
pub(in crate::backend::direct_wasm) use expression_traversal::*;
pub(in crate::backend::direct_wasm) use resolution::*;
pub(in crate::backend::direct_wasm) use statement_traversal::*;
