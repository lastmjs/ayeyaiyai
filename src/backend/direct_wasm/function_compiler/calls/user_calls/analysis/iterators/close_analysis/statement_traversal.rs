use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn collect_iterator_close_binding_names_from_statements(
        statements: &[Statement],
        names: &mut Vec<String>,
    ) {
        for statement in statements {
            match statement {
                Statement::Declaration { body }
                | Statement::Block { body }
                | Statement::Labeled { body, .. }
                | Statement::With { body, .. } => {
                    Self::collect_iterator_close_binding_names_from_statements(body, names);
                }
                Statement::Expression(expression)
                | Statement::Return(expression)
                | Statement::Throw(expression)
                | Statement::Yield { value: expression }
                | Statement::YieldDelegate { value: expression } => {
                    super::expression_traversal::collect_iterator_close_names_from_expression(
                        expression, names,
                    );
                }
                Statement::Var { value, .. }
                | Statement::Let { value, .. }
                | Statement::Assign { value, .. } => {
                    super::expression_traversal::collect_iterator_close_names_from_expression(
                        value, names,
                    );
                }
                Statement::AssignMember {
                    object,
                    property,
                    value,
                } => {
                    super::expression_traversal::collect_iterator_close_names_from_expression(
                        object, names,
                    );
                    super::expression_traversal::collect_iterator_close_names_from_expression(
                        property, names,
                    );
                    super::expression_traversal::collect_iterator_close_names_from_expression(
                        value, names,
                    );
                }
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    super::expression_traversal::collect_iterator_close_names_from_expression(
                        condition, names,
                    );
                    Self::collect_iterator_close_binding_names_from_statements(then_branch, names);
                    Self::collect_iterator_close_binding_names_from_statements(else_branch, names);
                }
                Statement::While {
                    condition,
                    break_hook,
                    body,
                    ..
                }
                | Statement::DoWhile {
                    condition,
                    break_hook,
                    body,
                    ..
                } => {
                    super::expression_traversal::collect_iterator_close_names_from_expression(
                        condition, names,
                    );
                    if let Some(break_hook) = break_hook {
                        super::expression_traversal::collect_iterator_close_names_from_expression(
                            break_hook, names,
                        );
                    }
                    Self::collect_iterator_close_binding_names_from_statements(body, names);
                }
                Statement::For {
                    init,
                    condition,
                    update,
                    break_hook,
                    body,
                    ..
                } => {
                    Self::collect_iterator_close_binding_names_from_statements(init, names);
                    if let Some(condition) = condition {
                        super::expression_traversal::collect_iterator_close_names_from_expression(
                            condition, names,
                        );
                    }
                    if let Some(update) = update {
                        super::expression_traversal::collect_iterator_close_names_from_expression(
                            update, names,
                        );
                    }
                    if let Some(break_hook) = break_hook {
                        super::expression_traversal::collect_iterator_close_names_from_expression(
                            break_hook, names,
                        );
                    }
                    Self::collect_iterator_close_binding_names_from_statements(body, names);
                }
                Statement::Try {
                    body,
                    catch_setup,
                    catch_body,
                    ..
                } => {
                    Self::collect_iterator_close_binding_names_from_statements(body, names);
                    Self::collect_iterator_close_binding_names_from_statements(catch_setup, names);
                    Self::collect_iterator_close_binding_names_from_statements(catch_body, names);
                }
                Statement::Switch {
                    discriminant,
                    cases,
                    ..
                } => {
                    super::expression_traversal::collect_iterator_close_names_from_expression(
                        discriminant,
                        names,
                    );
                    for case in cases {
                        if let Some(test) = &case.test {
                            super::expression_traversal::collect_iterator_close_names_from_expression(
                                test, names,
                            );
                        }
                        Self::collect_iterator_close_binding_names_from_statements(
                            &case.body, names,
                        );
                    }
                }
                Statement::Print { values } => {
                    for value in values {
                        super::expression_traversal::collect_iterator_close_names_from_expression(
                            value, names,
                        );
                    }
                }
                Statement::Break { .. } | Statement::Continue { .. } => {}
            }
        }
    }
}
