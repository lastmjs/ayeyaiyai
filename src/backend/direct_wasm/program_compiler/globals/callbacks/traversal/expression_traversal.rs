use super::*;

#[path = "expression_traversal/aggregate_literals.rs"]
mod aggregate_literals;
#[path = "expression_traversal/call_like.rs"]
mod call_like;
#[path = "expression_traversal/member_access.rs"]
mod member_access;
#[path = "expression_traversal/recursive_shapes.rs"]
mod recursive_shapes;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn collect_stateful_callback_bindings_from_expression(
        &self,
        expression: &Expression,
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
        value_bindings: &HashMap<String, Expression>,
        object_state: &HashMap<String, ObjectValueBinding>,
        overwrite_existing: bool,
    ) {
        if self.collect_stateful_callback_bindings_from_call_like(
            expression,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
            value_bindings,
            object_state,
            overwrite_existing,
        ) {
            return;
        }
        if self.collect_stateful_callback_bindings_from_aggregate_literals(
            expression,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
            value_bindings,
            object_state,
            overwrite_existing,
        ) {
            return;
        }
        if self.collect_stateful_callback_bindings_from_member_access(
            expression,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
            value_bindings,
            object_state,
            overwrite_existing,
        ) {
            return;
        }
        self.collect_stateful_callback_bindings_from_recursive_shapes(
            expression,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
            value_bindings,
            object_state,
            overwrite_existing,
        );
    }
}
