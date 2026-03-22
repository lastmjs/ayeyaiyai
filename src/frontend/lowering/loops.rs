use super::*;

impl Lowerer {
    pub(crate) fn lower_for_of_statement(
        &mut self,
        for_of_statement: &ForOfStmt,
        allow_return: bool,
    ) -> Result<Vec<Statement>> {
        let iterator_name = self.fresh_temporary_name("for_of_iter");
        let step_name = self.fresh_temporary_name("for_of_step");
        let done_name = self.fresh_temporary_name("for_of_done");
        let iterator_value =
            Expression::GetIterator(Box::new(self.lower_expression(&for_of_statement.right)?));
        let step_value = Expression::Call {
            callee: Box::new(Expression::Member {
                object: Box::new(Expression::Identifier(iterator_name.clone())),
                property: Box::new(Expression::String("next".to_string())),
            }),
            arguments: Vec::new(),
        };
        let step_done = Expression::Member {
            object: Box::new(Expression::Identifier(step_name.clone())),
            property: Box::new(Expression::String("done".to_string())),
        };
        let iterated_value = Expression::Member {
            object: Box::new(Expression::Identifier(step_name.clone())),
            property: Box::new(Expression::String("value".to_string())),
        };
        let break_hook = Expression::Conditional {
            condition: Box::new(Expression::Identifier(done_name.clone())),
            then_expression: Box::new(Expression::Undefined),
            else_expression: Box::new(Expression::IteratorClose(Box::new(Expression::Identifier(
                iterator_name.clone(),
            )))),
        };
        let binding = self.lower_for_of_binding(&for_of_statement.left, iterated_value)?;

        let mut body = vec![
            Statement::Let {
                name: step_name,
                mutable: true,
                value: step_value,
            },
            Statement::If {
                condition: step_done,
                then_branch: vec![
                    Statement::Assign {
                        name: done_name.clone(),
                        value: Expression::Bool(true),
                    },
                    Statement::Break { label: None },
                ],
                else_branch: Vec::new(),
            },
        ];
        body.extend(binding.per_iteration);
        body.extend(self.lower_block_or_statement(&for_of_statement.body, allow_return, true)?);

        let mut lowered = vec![Statement::Let {
            name: iterator_name,
            mutable: true,
            value: iterator_value,
        }];
        lowered.extend(binding.before_loop);
        lowered.push(Statement::Let {
            name: done_name,
            mutable: true,
            value: Expression::Bool(false),
        });
        lowered.push(Statement::While {
            labels: Vec::new(),
            condition: Expression::Bool(true),
            break_hook: Some(break_hook),
            body,
        });
        Ok(lowered)
    }

    pub(crate) fn lower_for_in_statement(
        &mut self,
        for_in_statement: &ForInStmt,
        allow_return: bool,
    ) -> Result<Vec<Statement>> {
        let target_name = self.fresh_temporary_name("for_in_target");
        let keys_name = self.fresh_temporary_name("for_in_keys");
        let index_name = self.fresh_temporary_name("for_in_index");
        let target_value = self.lower_expression(&for_in_statement.right)?;
        let target_expression = Expression::Identifier(target_name.clone());
        let enumerated_keys = Expression::EnumerateKeys(Box::new(target_expression.clone()));
        let current_key = Expression::Member {
            object: Box::new(Expression::Identifier(keys_name.clone())),
            property: Box::new(Expression::Identifier(index_name.clone())),
        };
        let binding = self.lower_for_of_binding(&for_in_statement.left, current_key.clone())?;

        let mut init = binding.before_loop;
        init.push(Statement::Let {
            name: target_name,
            mutable: false,
            value: target_value,
        });
        init.push(Statement::Let {
            name: keys_name.clone(),
            mutable: false,
            value: enumerated_keys,
        });
        init.push(Statement::Let {
            name: index_name.clone(),
            mutable: true,
            value: Expression::Number(0.0),
        });

        let mut body = vec![Statement::If {
            condition: Expression::Unary {
                op: UnaryOp::Not,
                expression: Box::new(Expression::Binary {
                    op: BinaryOp::In,
                    left: Box::new(current_key),
                    right: Box::new(target_expression),
                }),
            },
            then_branch: vec![Statement::Continue { label: None }],
            else_branch: Vec::new(),
        }];
        body.extend(binding.per_iteration);
        body.extend(self.lower_block_or_statement(&for_in_statement.body, allow_return, true)?);

        Ok(vec![Statement::For {
            labels: Vec::new(),
            init,
            per_iteration_bindings: Vec::new(),
            condition: Some(Expression::Binary {
                op: BinaryOp::LessThan,
                left: Box::new(Expression::Identifier(index_name.clone())),
                right: Box::new(Expression::Member {
                    object: Box::new(Expression::Identifier(keys_name)),
                    property: Box::new(Expression::String("length".to_string())),
                }),
            }),
            update: Some(Expression::Update {
                name: index_name,
                op: UpdateOp::Increment,
                prefix: false,
            }),
            break_hook: None,
            body,
        }])
    }

    pub(crate) fn lower_break_statement(
        &mut self,
        break_statement: &BreakStmt,
        allow_loop_control: bool,
    ) -> Result<Vec<Statement>> {
        if break_statement.label.is_none() {
            ensure!(allow_loop_control, "`break` is only supported inside loops");
        }

        Ok(vec![Statement::Break {
            label: break_statement
                .label
                .as_ref()
                .map(|label| label.sym.to_string()),
        }])
    }

    pub(crate) fn lower_continue_statement(
        &mut self,
        continue_statement: &ContinueStmt,
        allow_loop_control: bool,
    ) -> Result<Vec<Statement>> {
        if continue_statement.label.is_none() {
            ensure!(
                allow_loop_control,
                "`continue` is only supported inside loops"
            );
        }

        Ok(vec![Statement::Continue {
            label: continue_statement
                .label
                .as_ref()
                .map(|label| label.sym.to_string()),
        }])
    }

    pub(crate) fn lower_switch_statement(
        &mut self,
        switch_statement: &SwitchStmt,
        allow_return: bool,
        _allow_loop_control: bool,
    ) -> Result<Vec<Statement>> {
        let bindings = collect_switch_bindings(switch_statement)?;
        let binding_names = bindings.iter().cloned().collect::<HashSet<_>>();
        let cases = switch_statement
            .cases
            .iter()
            .map(|case| {
                Ok(SwitchCase {
                    test: case
                        .test
                        .as_deref()
                        .map(|expression| self.lower_expression(expression))
                        .transpose()?,
                    body: self.lower_switch_case_statements(
                        &case.cons,
                        allow_return,
                        true,
                        &binding_names,
                    )?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(vec![Statement::Switch {
            labels: Vec::new(),
            bindings,
            discriminant: self.lower_expression(&switch_statement.discriminant)?,
            cases,
        }])
    }

    pub(crate) fn lower_switch_case_statements(
        &mut self,
        statements: &[Stmt],
        allow_return: bool,
        allow_loop_control: bool,
        bindings: &HashSet<String>,
    ) -> Result<Vec<Statement>> {
        let mut lowered = Vec::new();

        for statement in statements {
            if let Stmt::Decl(Decl::Var(variable_declaration)) = statement
                && !matches!(variable_declaration.kind, VarDeclKind::Var)
            {
                lowered.extend(
                    self.lower_switch_case_lexical_declaration(variable_declaration, bindings)?,
                );
                continue;
            }

            lowered.extend(self.lower_statement(statement, allow_return, allow_loop_control)?);
        }

        Ok(lowered)
    }

    pub(crate) fn lower_switch_case_lexical_declaration(
        &mut self,
        variable_declaration: &swc_ecma_ast::VarDecl,
        bindings: &HashSet<String>,
    ) -> Result<Vec<Statement>> {
        let mut lowered = Vec::new();

        for declarator in &variable_declaration.decls {
            let mut names = Vec::new();
            collect_pattern_binding_names(&declarator.name, &mut names)?;
            if names.iter().any(|name| !bindings.contains(name)) {
                bail!("unsupported switch lexical binding");
            }

            let value = match declarator.init.as_deref() {
                Some(initializer) => self.lower_expression_with_name_hint(
                    initializer,
                    pattern_name_hint(&declarator.name),
                )?,
                None => Expression::Undefined,
            };

            if let Pat::Ident(identifier) = &declarator.name {
                lowered.push(Statement::Assign {
                    name: identifier.id.sym.to_string(),
                    value,
                });
                continue;
            }

            let temporary_name = self.fresh_temporary_name("switch_decl");
            lowered.push(Statement::Let {
                name: temporary_name.clone(),
                mutable: true,
                value,
            });
            self.lower_for_of_pattern_binding(
                &declarator.name,
                Expression::Identifier(temporary_name),
                ForOfPatternBindingKind::Assignment,
                &mut lowered,
            )?;
        }

        Ok(lowered)
    }

    pub(crate) fn lower_labeled_statement(
        &mut self,
        labeled_statement: &LabeledStmt,
        allow_return: bool,
        allow_loop_control: bool,
    ) -> Result<Vec<Statement>> {
        let label = labeled_statement.label.sym.to_string();
        let mut lowered = match &*labeled_statement.body {
            Stmt::Block(block) => vec![Statement::Labeled {
                labels: Vec::new(),
                body: self.lower_statements(&block.stmts, allow_return, allow_loop_control)?,
            }],
            statement => self.lower_statement(statement, allow_return, allow_loop_control)?,
        };

        self.attach_label_to_lowered(&mut lowered, label)?;
        Ok(lowered)
    }

    pub(crate) fn attach_label_to_lowered(
        &mut self,
        lowered: &mut Vec<Statement>,
        label: String,
    ) -> Result<()> {
        let single_statement = lowered.len() == 1;
        if let Some(last) = lowered.last_mut() {
            match last {
                Statement::For { labels, .. }
                | Statement::While { labels, .. }
                | Statement::DoWhile { labels, .. }
                | Statement::Switch { labels, .. } => {
                    labels.insert(0, label);
                    return Ok(());
                }
                Statement::Labeled { labels, .. } if single_statement => {
                    labels.insert(0, label);
                    return Ok(());
                }
                _ => {}
            }
        }

        if lowered.is_empty() {
            bail!("unsupported labeled statement")
        }

        let body = std::mem::take(lowered);
        lowered.push(Statement::Labeled {
            labels: vec![label],
            body,
        });
        Ok(())
    }
}
