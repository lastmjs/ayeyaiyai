use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn expression_depends_on_active_loop_assignment(
        &self,
        expression: &Expression,
    ) -> bool {
        let mut referenced_names = HashSet::new();
        collect_referenced_binding_names_from_expression(expression, &mut referenced_names);
        self.state
            .emission
            .control_flow
            .loop_stack
            .iter()
            .rev()
            .any(|loop_context| {
                referenced_names.iter().any(|name| {
                    loop_context.assigned_bindings.contains(name)
                        || scoped_binding_source_name(name).is_some_and(|source_name| {
                            loop_context.assigned_bindings.contains(source_name)
                        })
                })
            })
    }

    pub(in crate::backend::direct_wasm) fn if_condition_depends_on_active_loop_assignment(
        &self,
        condition: &Expression,
    ) -> bool {
        self.iterator_domain()
            .depends_on_active_loop_assignment(condition)
    }
}
