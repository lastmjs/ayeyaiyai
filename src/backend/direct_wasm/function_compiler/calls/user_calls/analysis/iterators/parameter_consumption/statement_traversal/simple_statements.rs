use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn collect_parameter_get_iterator_names_from_simple_statement(
        statement: &Statement,
        param_names: &HashSet<String>,
        consumed_names: &mut HashSet<String>,
    ) -> bool {
        match statement {
            Statement::Declaration { body }
            | Statement::Block { body }
            | Statement::Labeled { body, .. }
            | Statement::With { body, .. } => {
                Self::collect_parameter_get_iterator_names_from_statements(
                    body,
                    param_names,
                    consumed_names,
                );
                true
            }
            Statement::Expression(expression)
            | Statement::Return(expression)
            | Statement::Throw(expression)
            | Statement::Yield { value: expression }
            | Statement::YieldDelegate { value: expression } => {
                Self::collect_parameter_get_iterator_names_from_expression(
                    expression,
                    param_names,
                    consumed_names,
                );
                true
            }
            Statement::Var { value, .. }
            | Statement::Let { value, .. }
            | Statement::Assign { value, .. } => {
                Self::collect_parameter_get_iterator_names_from_expression(
                    value,
                    param_names,
                    consumed_names,
                );
                true
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                Self::collect_parameter_get_iterator_names_from_expression(
                    object,
                    param_names,
                    consumed_names,
                );
                Self::collect_parameter_get_iterator_names_from_expression(
                    property,
                    param_names,
                    consumed_names,
                );
                Self::collect_parameter_get_iterator_names_from_expression(
                    value,
                    param_names,
                    consumed_names,
                );
                true
            }
            Statement::Print { values } => {
                for value in values {
                    Self::collect_parameter_get_iterator_names_from_expression(
                        value,
                        param_names,
                        consumed_names,
                    );
                }
                true
            }
            _ => false,
        }
    }
}
