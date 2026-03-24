use std::collections::HashSet;

use anyhow::{Context, Result, bail};

use crate::ir::hir::{
    ArrayElement, CallArgument, Expression, FunctionDeclaration, ObjectEntry, Program, Statement,
    SwitchCase,
};

use super::{
    scope_stack::ScopeStack,
    support::{collect_statement_bindings, function_constructor_literal_source_parts},
};

mod function_constructor;

pub fn lower(program: Program) -> Result<Program> {
    StaticFunctionConstructorLowerer::new(&program).lower(program)
}

struct StaticFunctionConstructorLowerer {
    scopes: ScopeStack,
    global_scope: HashSet<String>,
    existing_function_names: HashSet<String>,
    synthetic_functions: Vec<FunctionDeclaration>,
    next_synthetic_function_id: usize,
}

impl StaticFunctionConstructorLowerer {
    fn new(program: &Program) -> Self {
        let mut global_scope = collect_statement_bindings(program.statements.iter())
            .into_iter()
            .collect::<HashSet<_>>();
        global_scope.extend(
            program
                .functions
                .iter()
                .filter(|function| function.register_global)
                .map(|function| function.name.clone()),
        );

        Self {
            scopes: ScopeStack::default(),
            global_scope,
            existing_function_names: program
                .functions
                .iter()
                .map(|function| function.name.clone())
                .collect(),
            synthetic_functions: Vec::new(),
            next_synthetic_function_id: 0,
        }
    }

    fn lower(mut self, mut program: Program) -> Result<Program> {
        self.scopes.push(self.global_scope.clone());
        program.statements = self.lower_statement_list(program.statements)?;

        let original_functions = std::mem::take(&mut program.functions);
        let mut lowered_functions = Vec::with_capacity(original_functions.len());
        for function in original_functions {
            lowered_functions.push(self.lower_function(function)?);
        }
        self.scopes.pop();

        lowered_functions.extend(self.synthetic_functions);
        program.functions = lowered_functions;
        Ok(program)
    }

    fn lower_function(&mut self, mut function: FunctionDeclaration) -> Result<FunctionDeclaration> {
        let mut function_scope = collect_statement_bindings(function.body.iter())
            .into_iter()
            .collect::<HashSet<_>>();
        function_scope.extend(
            function
                .params
                .iter()
                .map(|parameter| parameter.name.clone()),
        );
        if let Some(self_binding) = &function.self_binding {
            function_scope.insert(self_binding.clone());
        }
        function_scope.insert("arguments".to_string());

        self.scopes.push(function_scope);
        for parameter in &mut function.params {
            if let Some(default) = parameter.default.take() {
                parameter.default = Some(self.lower_expression(default)?);
            }
        }
        function.body = self.lower_statement_list(function.body)?;
        self.scopes.pop();
        Ok(function)
    }

    fn lower_synthetic_function(
        &mut self,
        mut function: FunctionDeclaration,
    ) -> Result<FunctionDeclaration> {
        function.top_level_binding = None;
        function.register_global = false;
        function.self_binding = None;

        let saved_scopes = std::mem::take(&mut self.scopes);
        self.scopes.push(self.global_scope.clone());
        let result = self.lower_function(function);
        self.scopes = saved_scopes;
        result
    }

    fn lower_statement_list(&mut self, statements: Vec<Statement>) -> Result<Vec<Statement>> {
        statements
            .into_iter()
            .map(|statement| self.lower_statement(statement))
            .collect()
    }

    fn lower_scoped_statement_list(
        &mut self,
        statements: Vec<Statement>,
        extra_bindings: impl IntoIterator<Item = String>,
    ) -> Result<Vec<Statement>> {
        let mut scope = collect_statement_bindings(statements.iter())
            .into_iter()
            .collect::<HashSet<_>>();
        scope.extend(extra_bindings);
        self.scopes.push(scope);
        let result = self.lower_statement_list(statements);
        self.scopes.pop();
        result
    }

    fn lower_statement(&mut self, statement: Statement) -> Result<Statement> {
        match statement {
            Statement::Block { body } => Ok(Statement::Block {
                body: self.lower_scoped_statement_list(body, [])?,
            }),
            Statement::Labeled { labels, body } => Ok(Statement::Labeled {
                labels,
                body: self.lower_scoped_statement_list(body, [])?,
            }),
            Statement::Var { name, value } => Ok(Statement::Var {
                name,
                value: self.lower_expression(value)?,
            }),
            Statement::Let {
                name,
                mutable,
                value,
            } => Ok(Statement::Let {
                name,
                mutable,
                value: self.lower_expression(value)?,
            }),
            Statement::Assign { name, value } => Ok(Statement::Assign {
                name,
                value: self.lower_expression(value)?,
            }),
            Statement::AssignMember {
                object,
                property,
                value,
            } => Ok(Statement::AssignMember {
                object: self.lower_expression(object)?,
                property: self.lower_expression(property)?,
                value: self.lower_expression(value)?,
            }),
            Statement::Print { values } => Ok(Statement::Print {
                values: values
                    .into_iter()
                    .map(|value| self.lower_expression(value))
                    .collect::<Result<Vec<_>>>()?,
            }),
            Statement::Expression(value) => {
                Ok(Statement::Expression(self.lower_expression(value)?))
            }
            Statement::Throw(value) => Ok(Statement::Throw(self.lower_expression(value)?)),
            Statement::Return(value) => Ok(Statement::Return(self.lower_expression(value)?)),
            Statement::Break { label } => Ok(Statement::Break { label }),
            Statement::Continue { label } => Ok(Statement::Continue { label }),
            Statement::Yield { value } => Ok(Statement::Yield {
                value: self.lower_expression(value)?,
            }),
            Statement::YieldDelegate { value } => Ok(Statement::YieldDelegate {
                value: self.lower_expression(value)?,
            }),
            Statement::With { object, body } => Ok(Statement::With {
                object: self.lower_expression(object)?,
                body: self.lower_scoped_statement_list(body, [])?,
            }),
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => Ok(Statement::If {
                condition: self.lower_expression(condition)?,
                then_branch: self.lower_scoped_statement_list(then_branch, [])?,
                else_branch: self.lower_scoped_statement_list(else_branch, [])?,
            }),
            Statement::Try {
                body,
                catch_binding,
                catch_setup,
                catch_body,
            } => {
                let body = self.lower_scoped_statement_list(body, [])?;

                let mut catch_bindings =
                    collect_statement_bindings(catch_setup.iter().chain(catch_body.iter()));
                if let Some(binding) = &catch_binding {
                    catch_bindings.push(binding.clone());
                }

                let catch_setup =
                    self.lower_scoped_statement_list(catch_setup, catch_bindings.iter().cloned())?;
                let catch_body = self.lower_scoped_statement_list(catch_body, catch_bindings)?;

                Ok(Statement::Try {
                    body,
                    catch_binding,
                    catch_setup,
                    catch_body,
                })
            }
            Statement::Switch {
                labels,
                bindings,
                discriminant,
                cases,
            } => {
                self.scopes
                    .push(bindings.iter().cloned().collect::<HashSet<_>>());
                let result = (|| -> Result<Vec<SwitchCase>> {
                    cases
                        .into_iter()
                        .map(|case| {
                            Ok(SwitchCase {
                                test: match case.test {
                                    Some(test) => Some(self.lower_expression(test)?),
                                    None => None,
                                },
                                body: self.lower_statement_list(case.body)?,
                            })
                        })
                        .collect()
                })();
                self.scopes.pop();

                Ok(Statement::Switch {
                    labels,
                    bindings,
                    discriminant: self.lower_expression(discriminant)?,
                    cases: result?,
                })
            }
            Statement::For {
                labels,
                init,
                per_iteration_bindings,
                condition,
                update,
                break_hook,
                body,
            } => {
                let mut loop_bindings = collect_statement_bindings(init.iter());
                loop_bindings.extend(per_iteration_bindings.iter().cloned());

                self.scopes
                    .push(loop_bindings.into_iter().collect::<HashSet<_>>());
                let result = (|| -> Result<_> {
                    Ok(Statement::For {
                        labels,
                        init: self.lower_statement_list(init)?,
                        per_iteration_bindings,
                        condition: match condition {
                            Some(condition) => Some(self.lower_expression(condition)?),
                            None => None,
                        },
                        update: match update {
                            Some(update) => Some(self.lower_expression(update)?),
                            None => None,
                        },
                        break_hook: match break_hook {
                            Some(break_hook) => Some(self.lower_expression(break_hook)?),
                            None => None,
                        },
                        body: self.lower_statement_list(body)?,
                    })
                })();
                self.scopes.pop();
                result
            }
            Statement::While {
                labels,
                condition,
                break_hook,
                body,
            } => Ok(Statement::While {
                labels,
                condition: self.lower_expression(condition)?,
                break_hook: match break_hook {
                    Some(break_hook) => Some(self.lower_expression(break_hook)?),
                    None => None,
                },
                body: self.lower_scoped_statement_list(body, [])?,
            }),
            Statement::DoWhile {
                labels,
                condition,
                break_hook,
                body,
            } => Ok(Statement::DoWhile {
                labels,
                condition: self.lower_expression(condition)?,
                break_hook: match break_hook {
                    Some(break_hook) => Some(self.lower_expression(break_hook)?),
                    None => None,
                },
                body: self.lower_scoped_statement_list(body, [])?,
            }),
        }
    }

    fn lower_expression(&mut self, expression: Expression) -> Result<Expression> {
        match expression {
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::NewTarget
            | Expression::Identifier(_)
            | Expression::This
            | Expression::Sent => Ok(expression),
            Expression::Array(elements) => Ok(Expression::Array(
                elements
                    .into_iter()
                    .map(|element| match element {
                        ArrayElement::Expression(expression) => {
                            Ok(ArrayElement::Expression(self.lower_expression(expression)?))
                        }
                        ArrayElement::Spread(expression) => {
                            Ok(ArrayElement::Spread(self.lower_expression(expression)?))
                        }
                    })
                    .collect::<Result<Vec<_>>>()?,
            )),
            Expression::Object(entries) => Ok(Expression::Object(
                entries
                    .into_iter()
                    .map(|entry| match entry {
                        ObjectEntry::Data { key, value } => Ok(ObjectEntry::Data {
                            key: self.lower_expression(key)?,
                            value: self.lower_expression(value)?,
                        }),
                        ObjectEntry::Getter { key, getter } => Ok(ObjectEntry::Getter {
                            key: self.lower_expression(key)?,
                            getter: self.lower_expression(getter)?,
                        }),
                        ObjectEntry::Setter { key, setter } => Ok(ObjectEntry::Setter {
                            key: self.lower_expression(key)?,
                            setter: self.lower_expression(setter)?,
                        }),
                        ObjectEntry::Spread(expression) => {
                            Ok(ObjectEntry::Spread(self.lower_expression(expression)?))
                        }
                    })
                    .collect::<Result<Vec<_>>>()?,
            )),
            Expression::Member { object, property } => Ok(Expression::Member {
                object: Box::new(self.lower_expression(*object)?),
                property: Box::new(self.lower_expression(*property)?),
            }),
            Expression::SuperMember { property } => Ok(Expression::SuperMember {
                property: Box::new(self.lower_expression(*property)?),
            }),
            Expression::Assign { name, value } => Ok(Expression::Assign {
                name,
                value: Box::new(self.lower_expression(*value)?),
            }),
            Expression::AssignMember {
                object,
                property,
                value,
            } => Ok(Expression::AssignMember {
                object: Box::new(self.lower_expression(*object)?),
                property: Box::new(self.lower_expression(*property)?),
                value: Box::new(self.lower_expression(*value)?),
            }),
            Expression::AssignSuperMember { property, value } => {
                Ok(Expression::AssignSuperMember {
                    property: Box::new(self.lower_expression(*property)?),
                    value: Box::new(self.lower_expression(*value)?),
                })
            }
            Expression::Await(expression) => Ok(Expression::Await(Box::new(
                self.lower_expression(*expression)?,
            ))),
            Expression::EnumerateKeys(expression) => Ok(Expression::EnumerateKeys(Box::new(
                self.lower_expression(*expression)?,
            ))),
            Expression::GetIterator(expression) => Ok(Expression::GetIterator(Box::new(
                self.lower_expression(*expression)?,
            ))),
            Expression::IteratorClose(expression) => Ok(Expression::IteratorClose(Box::new(
                self.lower_expression(*expression)?,
            ))),
            Expression::Unary { op, expression } => Ok(Expression::Unary {
                op,
                expression: Box::new(self.lower_expression(*expression)?),
            }),
            Expression::Binary { op, left, right } => Ok(Expression::Binary {
                op,
                left: Box::new(self.lower_expression(*left)?),
                right: Box::new(self.lower_expression(*right)?),
            }),
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => Ok(Expression::Conditional {
                condition: Box::new(self.lower_expression(*condition)?),
                then_expression: Box::new(self.lower_expression(*then_expression)?),
                else_expression: Box::new(self.lower_expression(*else_expression)?),
            }),
            Expression::Sequence(expressions) => Ok(Expression::Sequence(
                expressions
                    .into_iter()
                    .map(|expression| self.lower_expression(expression))
                    .collect::<Result<Vec<_>>>()?,
            )),
            Expression::Call { callee, arguments } => {
                let callee = self.lower_expression(*callee)?;
                let arguments = self.lower_arguments(arguments)?;
                if let Some(lowered) =
                    self.try_lower_static_function_constructor(&callee, &arguments)?
                {
                    return Ok(lowered);
                }
                Ok(Expression::Call {
                    callee: Box::new(callee),
                    arguments,
                })
            }
            Expression::SuperCall { callee, arguments } => Ok(Expression::SuperCall {
                callee: Box::new(self.lower_expression(*callee)?),
                arguments: self.lower_arguments(arguments)?,
            }),
            Expression::New { callee, arguments } => {
                let callee = self.lower_expression(*callee)?;
                let arguments = self.lower_arguments(arguments)?;
                if let Some(lowered) =
                    self.try_lower_static_function_constructor(&callee, &arguments)?
                {
                    return Ok(lowered);
                }
                Ok(Expression::New {
                    callee: Box::new(callee),
                    arguments,
                })
            }
            Expression::Update { .. } => Ok(expression),
        }
    }

    fn lower_arguments(&mut self, arguments: Vec<CallArgument>) -> Result<Vec<CallArgument>> {
        arguments
            .into_iter()
            .map(|argument| match argument {
                CallArgument::Expression(expression) => {
                    Ok(CallArgument::Expression(self.lower_expression(expression)?))
                }
                CallArgument::Spread(expression) => {
                    Ok(CallArgument::Spread(self.lower_expression(expression)?))
                }
            })
            .collect()
    }
}
