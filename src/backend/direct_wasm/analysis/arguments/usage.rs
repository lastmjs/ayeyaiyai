use super::*;

#[path = "usage/collection.rs"]
mod collection;
#[path = "usage/expression_traversal.rs"]
mod expression_traversal;
#[path = "usage/statement_traversal.rs"]
mod statement_traversal;

pub(in crate::backend::direct_wasm) use self::{
    collection::*, expression_traversal::*, statement_traversal::*,
};
