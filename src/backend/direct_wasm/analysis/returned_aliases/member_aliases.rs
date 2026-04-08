#[path = "member_aliases/collection.rs"]
mod collection;
#[path = "member_aliases/expression_traversal.rs"]
mod expression_traversal;
#[path = "member_aliases/resolution.rs"]
mod resolution;
#[path = "member_aliases/statement_traversal.rs"]
mod statement_traversal;

pub(in crate::backend::direct_wasm) use self::{
    collection::collect_returned_member_local_aliases,
    expression_traversal::collect_returned_member_local_aliases_from_expression,
    resolution::resolve_returned_member_local_alias_expression,
    statement_traversal::collect_returned_member_local_aliases_from_statement,
};
