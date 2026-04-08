use super::*;

#[path = "expression_traversal/direct_detection.rs"]
mod direct_detection;
#[path = "expression_traversal/recursive_shapes.rs"]
mod recursive_shapes;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn collect_parameter_get_iterator_names_from_expression(
        expression: &Expression,
        param_names: &HashSet<String>,
        consumed_names: &mut HashSet<String>,
    ) {
        Self::collect_direct_parameter_get_iterator_name(expression, param_names, consumed_names);
        Self::collect_parameter_get_iterator_names_from_children(
            expression,
            param_names,
            consumed_names,
        );
    }
}
