#[path = "expression_traversal/call_bindings.rs"]
mod call_bindings;
#[path = "expression_traversal/recursive_shapes.rs"]
mod recursive_shapes;

pub(in crate::backend::direct_wasm) use self::call_bindings::collect_returned_member_function_bindings_from_expression;
