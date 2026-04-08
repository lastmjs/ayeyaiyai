use super::*;
mod eval_calls;
mod recursive_shapes;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn collect_static_direct_eval_assigned_nonlocal_names_from_expression(
        &self,
        expression: &Expression,
        current_function_name: Option<&str>,
        names: &mut HashSet<String>,
    ) {
        match expression {
            Expression::Call { callee, arguments } if matches!(callee.as_ref(), Expression::Identifier(name) if name == "eval") =>
            {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_eval_call(
                    arguments,
                    current_function_name,
                    names,
                );
            }
            _ => self.collect_static_direct_eval_assigned_nonlocal_names_from_expression_recursive(
                expression,
                current_function_name,
                names,
            ),
        }
    }
}
