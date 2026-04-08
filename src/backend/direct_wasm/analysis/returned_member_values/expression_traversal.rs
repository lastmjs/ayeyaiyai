#[path = "expression_traversal/assignment_updates.rs"]
mod assignment_updates;
#[path = "expression_traversal/define_property.rs"]
mod define_property;
#[path = "expression_traversal/recursive_shapes.rs"]
mod recursive_shapes;

pub(in crate::backend::direct_wasm) use self::assignment_updates::collect_returned_member_value_bindings_from_expression;
