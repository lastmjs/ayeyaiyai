use super::*;
use crate::ir::hir::SwitchCase;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn substitute_static_constructor_new_target_statement(
        statement: &Statement,
    ) -> Statement {
        match statement {
            Statement::Declaration { body } => Statement::Declaration {
                body: body
                    .iter()
                    .map(Self::substitute_static_constructor_new_target_statement)
                    .collect(),
            },
            Statement::Block { body } => Statement::Block {
                body: body
                    .iter()
                    .map(Self::substitute_static_constructor_new_target_statement)
                    .collect(),
            },
            Statement::Labeled { labels, body } => Statement::Labeled {
                labels: labels.clone(),
                body: body
                    .iter()
                    .map(Self::substitute_static_constructor_new_target_statement)
                    .collect(),
            },
            Statement::Var { name, value } => Statement::Var {
                name: name.clone(),
                value: Self::substitute_static_constructor_new_target_expression(value),
            },
            Statement::Let {
                name,
                mutable,
                value,
            } => Statement::Let {
                name: name.clone(),
                mutable: *mutable,
                value: Self::substitute_static_constructor_new_target_expression(value),
            },
            Statement::Assign { name, value } => Statement::Assign {
                name: name.clone(),
                value: Self::substitute_static_constructor_new_target_expression(value),
            },
            Statement::AssignMember {
                object,
                property,
                value,
            } => Statement::AssignMember {
                object: Self::substitute_static_constructor_new_target_expression(object),
                property: Self::substitute_static_constructor_new_target_expression(property),
                value: Self::substitute_static_constructor_new_target_expression(value),
            },
            Statement::Print { values } => Statement::Print {
                values: values
                    .iter()
                    .map(Self::substitute_static_constructor_new_target_expression)
                    .collect(),
            },
            Statement::Expression(expression) => Statement::Expression(
                Self::substitute_static_constructor_new_target_expression(expression),
            ),
            Statement::Throw(expression) => Statement::Throw(
                Self::substitute_static_constructor_new_target_expression(expression),
            ),
            Statement::Return(expression) => Statement::Return(
                Self::substitute_static_constructor_new_target_expression(expression),
            ),
            Statement::With { object, body } => Statement::With {
                object: Self::substitute_static_constructor_new_target_expression(object),
                body: body
                    .iter()
                    .map(Self::substitute_static_constructor_new_target_statement)
                    .collect(),
            },
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => Statement::If {
                condition: Self::substitute_static_constructor_new_target_expression(condition),
                then_branch: then_branch
                    .iter()
                    .map(Self::substitute_static_constructor_new_target_statement)
                    .collect(),
                else_branch: else_branch
                    .iter()
                    .map(Self::substitute_static_constructor_new_target_statement)
                    .collect(),
            },
            Statement::Try {
                body,
                catch_binding,
                catch_setup,
                catch_body,
            } => Statement::Try {
                body: body
                    .iter()
                    .map(Self::substitute_static_constructor_new_target_statement)
                    .collect(),
                catch_binding: catch_binding.clone(),
                catch_setup: catch_setup
                    .iter()
                    .map(Self::substitute_static_constructor_new_target_statement)
                    .collect(),
                catch_body: catch_body
                    .iter()
                    .map(Self::substitute_static_constructor_new_target_statement)
                    .collect(),
            },
            Statement::Switch {
                labels,
                bindings,
                discriminant,
                cases,
            } => Statement::Switch {
                labels: labels.clone(),
                bindings: bindings.clone(),
                discriminant: Self::substitute_static_constructor_new_target_expression(
                    discriminant,
                ),
                cases: cases
                    .iter()
                    .map(|case| SwitchCase {
                        test: case
                            .test
                            .as_ref()
                            .map(Self::substitute_static_constructor_new_target_expression),
                        body: case
                            .body
                            .iter()
                            .map(Self::substitute_static_constructor_new_target_statement)
                            .collect(),
                    })
                    .collect(),
            },
            Statement::For {
                labels,
                init,
                per_iteration_bindings,
                condition,
                update,
                break_hook,
                body,
            } => Statement::For {
                labels: labels.clone(),
                init: init
                    .iter()
                    .map(Self::substitute_static_constructor_new_target_statement)
                    .collect(),
                per_iteration_bindings: per_iteration_bindings.clone(),
                condition: condition
                    .as_ref()
                    .map(Self::substitute_static_constructor_new_target_expression),
                update: update
                    .as_ref()
                    .map(Self::substitute_static_constructor_new_target_expression),
                break_hook: break_hook
                    .as_ref()
                    .map(Self::substitute_static_constructor_new_target_expression),
                body: body
                    .iter()
                    .map(Self::substitute_static_constructor_new_target_statement)
                    .collect(),
            },
            Statement::While {
                labels,
                condition,
                break_hook,
                body,
            } => Statement::While {
                labels: labels.clone(),
                condition: Self::substitute_static_constructor_new_target_expression(condition),
                break_hook: break_hook
                    .as_ref()
                    .map(Self::substitute_static_constructor_new_target_expression),
                body: body
                    .iter()
                    .map(Self::substitute_static_constructor_new_target_statement)
                    .collect(),
            },
            Statement::DoWhile {
                labels,
                condition,
                break_hook,
                body,
            } => Statement::DoWhile {
                labels: labels.clone(),
                condition: Self::substitute_static_constructor_new_target_expression(condition),
                break_hook: break_hook
                    .as_ref()
                    .map(Self::substitute_static_constructor_new_target_expression),
                body: body
                    .iter()
                    .map(Self::substitute_static_constructor_new_target_statement)
                    .collect(),
            },
            Statement::Break { label } => Statement::Break {
                label: label.clone(),
            },
            Statement::Continue { label } => Statement::Continue {
                label: label.clone(),
            },
            Statement::Yield { value } => Statement::Yield {
                value: Self::substitute_static_constructor_new_target_expression(value),
            },
            Statement::YieldDelegate { value } => Statement::YieldDelegate {
                value: Self::substitute_static_constructor_new_target_expression(value),
            },
        }
    }
}
