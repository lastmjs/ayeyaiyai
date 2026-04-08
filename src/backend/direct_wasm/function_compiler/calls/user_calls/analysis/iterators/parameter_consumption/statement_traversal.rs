use super::*;

#[path = "statement_traversal/control_flow.rs"]
mod control_flow;
#[path = "statement_traversal/simple_statements.rs"]
mod simple_statements;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn collect_parameter_get_iterator_names_from_statements(
        statements: &[Statement],
        param_names: &HashSet<String>,
        consumed_names: &mut HashSet<String>,
    ) {
        for statement in statements {
            if Self::collect_parameter_get_iterator_names_from_simple_statement(
                statement,
                param_names,
                consumed_names,
            ) {
                continue;
            }
            Self::collect_parameter_get_iterator_names_from_control_flow_statement(
                statement,
                param_names,
                consumed_names,
            );
        }
    }
}
