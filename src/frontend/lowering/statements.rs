use super::*;

impl Lowerer {
    pub(crate) fn lower_statements(
        &mut self,
        statements: &[Stmt],
        allow_return: bool,
        allow_loop_control: bool,
    ) -> Result<Vec<Statement>> {
        let scope_bindings = collect_direct_statement_lexical_bindings(statements)?;
        self.push_binding_scope(scope_bindings);
        let lowered = (|| -> Result<Vec<Statement>> {
            let mut lowered = Vec::new();

            for statement in statements {
                lowered.extend(self.lower_statement(
                    statement,
                    allow_return,
                    allow_loop_control,
                )?);
            }

            Ok(lowered)
        })();
        self.pop_binding_scope();
        lowered
    }

    pub(crate) fn lower_statement(
        &mut self,
        statement: &Stmt,
        allow_return: bool,
        allow_loop_control: bool,
    ) -> Result<Vec<Statement>> {
        match statement {
            Stmt::Decl(Decl::Var(variable_declaration)) => {
                self.lower_variable_declaration(variable_declaration)
            }
            Stmt::Decl(Decl::Fn(function_declaration)) => {
                self.lower_nested_function_declaration(function_declaration)
            }
            Stmt::Decl(Decl::Class(class_declaration)) => {
                self.lower_class_declaration(class_declaration)
            }
            Stmt::Expr(ExprStmt { expr, .. }) => self.lower_expression_statement(expr),
            Stmt::Block(block) => Ok(vec![Statement::Block {
                body: self.lower_statements(&block.stmts, allow_return, allow_loop_control)?,
            }]),
            Stmt::If(if_statement) => Ok(vec![Statement::If {
                condition: self.lower_expression(&if_statement.test)?,
                then_branch: self.lower_block_or_statement(
                    &if_statement.cons,
                    allow_return,
                    allow_loop_control,
                )?,
                else_branch: self.lower_optional_else(
                    if_statement.alt.as_deref(),
                    allow_return,
                    allow_loop_control,
                )?,
            }]),
            Stmt::Switch(switch_statement) => {
                self.lower_switch_statement(switch_statement, allow_return, allow_loop_control)
            }
            Stmt::For(for_statement) => Ok(vec![Statement::For {
                labels: Vec::new(),
                init: match &for_statement.init {
                    Some(VarDeclOrExpr::VarDecl(variable_declaration)) => {
                        self.lower_variable_declaration(variable_declaration)?
                    }
                    Some(VarDeclOrExpr::Expr(expression)) => {
                        self.lower_expression_statement(expression)?
                    }
                    None => Vec::new(),
                },
                condition: for_statement
                    .test
                    .as_deref()
                    .map(|expression| self.lower_expression(expression))
                    .transpose()?,
                update: for_statement
                    .update
                    .as_deref()
                    .map(|expression| self.lower_expression(expression))
                    .transpose()?,
                per_iteration_bindings: for_statement
                    .init
                    .as_ref()
                    .map(collect_for_per_iteration_bindings)
                    .transpose()?
                    .unwrap_or_default(),
                break_hook: None,
                body: self.lower_block_or_statement(&for_statement.body, allow_return, true)?,
            }]),
            Stmt::ForOf(for_of_statement) => {
                self.lower_for_of_statement(for_of_statement, allow_return)
            }
            Stmt::ForIn(for_in_statement) => {
                self.lower_for_in_statement(for_in_statement, allow_return)
            }
            Stmt::DoWhile(do_while_statement) => Ok(vec![Statement::DoWhile {
                labels: Vec::new(),
                condition: self.lower_expression(&do_while_statement.test)?,
                break_hook: None,
                body: self.lower_block_or_statement(
                    &do_while_statement.body,
                    allow_return,
                    true,
                )?,
            }]),
            Stmt::With(with_statement) => Ok(vec![Statement::With {
                object: self.lower_expression(&with_statement.obj)?,
                body: self.lower_block_or_statement(
                    &with_statement.body,
                    allow_return,
                    allow_loop_control,
                )?,
            }]),
            Stmt::While(while_statement) => Ok(vec![Statement::While {
                labels: Vec::new(),
                condition: self.lower_expression(&while_statement.test)?,
                break_hook: None,
                body: self.lower_block_or_statement(&while_statement.body, allow_return, true)?,
            }]),
            Stmt::Throw(throw_statement) => Ok(vec![Statement::Throw(
                self.lower_expression(&throw_statement.arg)?,
            )]),
            Stmt::Try(try_statement) => {
                self.lower_try_statement(try_statement, allow_return, allow_loop_control)
            }
            Stmt::Return(return_statement) => {
                ensure!(allow_return, "`return` is only supported inside functions");
                Ok(vec![Statement::Return(
                    match return_statement.arg.as_deref() {
                        Some(expression) => self.lower_expression(expression)?,
                        None => Expression::Undefined,
                    },
                )])
            }
            Stmt::Break(break_statement) => {
                self.lower_break_statement(break_statement, allow_loop_control)
            }
            Stmt::Continue(continue_statement) => {
                self.lower_continue_statement(continue_statement, allow_loop_control)
            }
            Stmt::Labeled(labeled_statement) => {
                self.lower_labeled_statement(labeled_statement, allow_return, allow_loop_control)
            }
            Stmt::Empty(_) => Ok(Vec::new()),
            _ => bail!("unsupported statement: {statement:?}"),
        }
    }

    pub(crate) fn lower_try_statement(
        &mut self,
        try_statement: &swc_ecma_ast::TryStmt,
        allow_return: bool,
        allow_loop_control: bool,
    ) -> Result<Vec<Statement>> {
        let lowered_body =
            self.lower_statements(&try_statement.block.stmts, allow_return, allow_loop_control)?;
        let lowered_handler = try_statement
            .handler
            .as_ref()
            .map(|handler| self.lower_catch_clause(handler, allow_return, allow_loop_control))
            .transpose()?;

        if let Some(finalizer) = &try_statement.finalizer {
            let threw_name = self.fresh_temporary_name("finally_threw");
            let error_name = self.fresh_temporary_name("finally_error");
            let outer_catch_name = self.fresh_temporary_name("finally_catch");
            let mut statements = vec![
                Statement::Let {
                    name: threw_name.clone(),
                    mutable: true,
                    value: Expression::Bool(false),
                },
                Statement::Let {
                    name: error_name.clone(),
                    mutable: true,
                    value: Expression::Undefined,
                },
            ];

            let protected_body =
                if let Some((catch_binding, catch_setup, catch_body)) = lowered_handler {
                    vec![Statement::Try {
                        body: lowered_body,
                        catch_binding,
                        catch_setup,
                        catch_body,
                    }]
                } else {
                    lowered_body
                };

            statements.push(Statement::Try {
                body: protected_body,
                catch_binding: Some(outer_catch_name.clone()),
                catch_setup: Vec::new(),
                catch_body: vec![
                    Statement::Assign {
                        name: threw_name.clone(),
                        value: Expression::Bool(true),
                    },
                    Statement::Assign {
                        name: error_name.clone(),
                        value: Expression::Identifier(outer_catch_name),
                    },
                ],
            });
            statements.extend(self.lower_statements(
                &finalizer.stmts,
                allow_return,
                allow_loop_control,
            )?);
            statements.push(Statement::If {
                condition: Expression::Identifier(threw_name),
                then_branch: vec![Statement::Throw(Expression::Identifier(error_name))],
                else_branch: Vec::new(),
            });
            return Ok(statements);
        }

        let (catch_binding, catch_setup, catch_body) =
            lowered_handler.context("`try` without `catch` is not supported yet")?;
        Ok(vec![Statement::Try {
            body: lowered_body,
            catch_binding,
            catch_setup,
            catch_body,
        }])
    }

    pub(crate) fn lower_catch_clause(
        &mut self,
        handler: &swc_ecma_ast::CatchClause,
        allow_return: bool,
        allow_loop_control: bool,
    ) -> Result<(Option<String>, Vec<Statement>, Vec<Statement>)> {
        let mut scope_bindings = Vec::new();
        if let Some(pattern) = handler.param.as_ref() {
            collect_pattern_binding_names(pattern, &mut scope_bindings)?;
        }

        self.push_binding_scope(scope_bindings);
        let lowered = (|| -> Result<(Option<String>, Vec<Statement>, Vec<Statement>)> {
            let (catch_binding, catch_setup) = match handler.param.as_ref() {
                Some(Pat::Ident(binding)) => (
                    Some(self.resolve_binding_name(binding.id.sym.as_ref())),
                    Vec::new(),
                ),
                None => (None, Vec::new()),
                Some(pattern) => {
                    let temporary_name = self.fresh_temporary_name("catch");
                    let mut setup = Vec::new();
                    self.lower_for_of_pattern_binding(
                        pattern,
                        Expression::Identifier(temporary_name.clone()),
                        ForOfPatternBindingKind::Lexical { mutable: true },
                        &mut setup,
                    )?;
                    (Some(temporary_name), setup)
                }
            };

            Ok((
                catch_binding,
                catch_setup,
                self.lower_statements(&handler.body.stmts, allow_return, allow_loop_control)?,
            ))
        })();
        self.pop_binding_scope();
        lowered
    }

    pub(crate) fn lower_expression_statement(
        &mut self,
        expression: &Expr,
    ) -> Result<Vec<Statement>> {
        if let Some(arguments) = console_log_arguments(expression) {
            return Ok(vec![Statement::Print {
                values: arguments
                    .iter()
                    .map(|argument| self.lower_expression(&argument.expr))
                    .collect::<Result<Vec<_>>>()?,
            }]);
        }

        if let Some(call) = assert_throws_call(expression) {
            return self.lower_assert_throws_statement(call);
        }

        if let Expr::Assign(assignment) = expression {
            let target = self.lower_assignment_target(&assignment.left)?;

            if assignment.op == AssignOp::Assign {
                let value = match &target {
                    AssignmentTarget::Identifier(name) => {
                        self.lower_expression_with_name_hint(&assignment.right, Some(name))?
                    }
                    AssignmentTarget::Member { .. } | AssignmentTarget::SuperMember { .. } => {
                        self.lower_expression(&assignment.right)?
                    }
                };
                return Ok(vec![target.into_statement(value)]);
            }

            let operator = assignment
                .op
                .to_update()
                .context("unsupported assignment operator")?;

            let right = match &target {
                AssignmentTarget::Identifier(name) => {
                    self.lower_expression_with_name_hint(&assignment.right, Some(name))?
                }
                AssignmentTarget::Member { .. } | AssignmentTarget::SuperMember { .. } => {
                    self.lower_expression(&assignment.right)?
                }
            };
            let binary = match &target {
                AssignmentTarget::Identifier(name) => Expression::Binary {
                    op: lower_binary_operator(operator)?,
                    left: Box::new(Expression::Identifier(name.clone())),
                    right: Box::new(right),
                },
                AssignmentTarget::Member { object, property } => Expression::Binary {
                    op: lower_binary_operator(operator)?,
                    left: Box::new(Expression::Member {
                        object: Box::new(object.clone()),
                        property: Box::new(property.clone()),
                    }),
                    right: Box::new(right),
                },
                AssignmentTarget::SuperMember { property } => Expression::Binary {
                    op: lower_binary_operator(operator)?,
                    left: Box::new(Expression::SuperMember {
                        property: Box::new(property.clone()),
                    }),
                    right: Box::new(right),
                },
            };

            return Ok(vec![target.into_statement(binary)]);
        }

        Ok(vec![Statement::Expression(
            self.lower_expression(expression)?,
        )])
    }

    pub(crate) fn lower_assert_throws_statement(
        &mut self,
        call: &swc_ecma_ast::CallExpr,
    ) -> Result<Vec<Statement>> {
        ensure!(
            call.args.len() >= 2,
            "__ayyAssertThrows expects at least two arguments"
        );
        ensure!(
            call.args.iter().all(|argument| argument.spread.is_none()),
            "__ayyAssertThrows does not support spread arguments"
        );

        let callback_name = self.fresh_temporary_name("assert_throws_callback");
        let callback_value =
            self.lower_expression_with_name_hint(&call.args[1].expr, Some(&callback_name))?;
        let caught_name = self.fresh_temporary_name("assert_throws_caught");

        let mut lowered = Vec::new();
        lowered.push(Statement::Let {
            name: callback_name.clone(),
            mutable: false,
            value: callback_value,
        });
        lowered.push(Statement::Let {
            name: caught_name.clone(),
            mutable: true,
            value: Expression::Bool(false),
        });
        lowered.push(Statement::Try {
            body: vec![Statement::Expression(Expression::Call {
                callee: Box::new(Expression::Identifier(callback_name)),
                arguments: Vec::new(),
            })],
            catch_binding: None,
            catch_setup: Vec::new(),
            catch_body: vec![Statement::Assign {
                name: caught_name.clone(),
                value: Expression::Bool(true),
            }],
        });
        lowered.push(Statement::If {
            condition: Expression::Binary {
                op: BinaryOp::Equal,
                left: Box::new(Expression::Identifier(caught_name)),
                right: Box::new(Expression::Bool(false)),
            },
            then_branch: vec![Statement::Throw(Expression::Undefined)],
            else_branch: Vec::new(),
        });

        Ok(lowered)
    }

    pub(crate) fn lower_block_or_statement(
        &mut self,
        statement: &Stmt,
        allow_return: bool,
        allow_loop_control: bool,
    ) -> Result<Vec<Statement>> {
        match statement {
            Stmt::Block(BlockStmt { stmts, .. }) => Ok(vec![Statement::Block {
                body: self.lower_statements(stmts, allow_return, allow_loop_control)?,
            }]),
            other => self.lower_statement(other, allow_return, allow_loop_control),
        }
    }

    pub(crate) fn lower_optional_else(
        &mut self,
        statement: Option<&Stmt>,
        allow_return: bool,
        allow_loop_control: bool,
    ) -> Result<Vec<Statement>> {
        match statement {
            Some(statement) => {
                self.lower_block_or_statement(statement, allow_return, allow_loop_control)
            }
            None => Ok(Vec::new()),
        }
    }
}
