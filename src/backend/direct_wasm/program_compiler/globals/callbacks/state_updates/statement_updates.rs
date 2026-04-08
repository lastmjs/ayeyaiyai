use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn update_parameter_binding_state_from_statement(
        &self,
        statement: &Statement,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) {
        match statement {
            Statement::Declaration { body }
            | Statement::Block { body }
            | Statement::Labeled { body, .. } => {
                for statement in body {
                    self.update_parameter_binding_state_from_statement(
                        statement,
                        value_bindings,
                        object_bindings,
                    );
                }
            }
            Statement::Var { name, value } | Statement::Let { name, value, .. } => self
                .update_parameter_binding_state_for_value(
                    name,
                    value,
                    value_bindings,
                    object_bindings,
                ),
            Statement::Assign { name, value } => self.update_parameter_binding_state_for_value(
                name,
                value,
                value_bindings,
                object_bindings,
            ),
            Statement::AssignMember {
                object,
                property,
                value,
            } => self.update_parameter_binding_state_from_member_assignment(
                object,
                property,
                value,
                value_bindings,
                object_bindings,
            ),
            Statement::Expression(expression) => self
                .update_parameter_binding_state_from_expression(
                    expression,
                    value_bindings,
                    object_bindings,
                ),
            _ => {}
        }
    }
}
