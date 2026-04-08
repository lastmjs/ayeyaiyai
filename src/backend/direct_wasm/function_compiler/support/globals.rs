use super::*;

#[path = "globals/expression_traversal.rs"]
mod expression_traversal;
#[path = "globals/statement_traversal.rs"]
mod statement_traversal;

pub(in crate::backend::direct_wasm) use expression_traversal::collect_implicit_globals_from_expression;
pub(in crate::backend::direct_wasm) use statement_traversal::collect_implicit_globals_from_statements;
