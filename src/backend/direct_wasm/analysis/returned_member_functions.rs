use super::*;

#[path = "returned_member_functions/collection.rs"]
mod collection;
#[path = "returned_member_functions/expression_traversal.rs"]
mod expression_traversal;
#[path = "returned_member_functions/resolution.rs"]
mod resolution;
#[path = "returned_member_functions/statement_traversal.rs"]
mod statement_traversal;

pub(in crate::backend::direct_wasm) use collection::*;
pub(in crate::backend::direct_wasm) use expression_traversal::*;
pub(in crate::backend::direct_wasm) use resolution::*;
pub(in crate::backend::direct_wasm) use statement_traversal::*;
