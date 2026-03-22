use super::*;

impl Lowerer {
    pub(super) fn lower_for_of_binding(
        &mut self,
        head: &ForHead,
        value: Expression,
    ) -> Result<ForOfBinding> {
        match head {
            ForHead::VarDecl(variable_declaration) => {
                ensure!(
                    variable_declaration.decls.len() == 1,
                    "for-of declarations with multiple bindings are not supported yet"
                );
                let pattern = &variable_declaration.decls[0].name;
                let mut binding = ForOfBinding {
                    before_loop: Vec::new(),
                    per_iteration: Vec::new(),
                };
                let binding_kind = match variable_declaration.kind {
                    VarDeclKind::Var => ForOfPatternBindingKind::Var,
                    VarDeclKind::Let => ForOfPatternBindingKind::Lexical { mutable: true },
                    VarDeclKind::Const => ForOfPatternBindingKind::Lexical { mutable: false },
                };

                if matches!(variable_declaration.kind, VarDeclKind::Var) {
                    let mut names = Vec::new();
                    collect_for_of_binding_names(pattern, &mut names)?;
                    binding.before_loop = names
                        .into_iter()
                        .map(|name| Statement::Var {
                            name,
                            value: Expression::Undefined,
                        })
                        .collect();
                }

                self.lower_for_of_pattern_binding(
                    pattern,
                    value,
                    binding_kind,
                    &mut binding.per_iteration,
                )?;

                Ok(binding)
            }
            ForHead::Pat(pattern) => {
                let mut binding = ForOfBinding {
                    before_loop: Vec::new(),
                    per_iteration: Vec::new(),
                };
                self.lower_for_of_pattern_binding(
                    pattern,
                    value,
                    ForOfPatternBindingKind::Assignment,
                    &mut binding.per_iteration,
                )?;
                Ok(binding)
            }
            ForHead::UsingDecl(_) => bail!("using declarations are not supported yet"),
        }
    }

    pub(super) fn lower_for_of_pattern_binding(
        &mut self,
        pattern: &Pat,
        value: Expression,
        binding_kind: ForOfPatternBindingKind,
        statements: &mut Vec<Statement>,
    ) -> Result<()> {
        match pattern {
            Pat::Ident(identifier) => {
                let name = self.resolve_binding_name(identifier.id.sym.as_ref());
                statements.push(match binding_kind {
                    ForOfPatternBindingKind::Var => Statement::Var { name, value },
                    ForOfPatternBindingKind::Assignment => Statement::Assign { name, value },
                    ForOfPatternBindingKind::Lexical { mutable } => Statement::Let {
                        name,
                        mutable,
                        value,
                    },
                })
            }
            Pat::Expr(expression) => {
                ensure!(
                    matches!(binding_kind, ForOfPatternBindingKind::Assignment),
                    "unsupported declaration binding pattern"
                );
                let target = self.lower_for_of_expression_target(expression)?;
                statements.push(target.into_statement(value));
            }
            Pat::Assign(assign) => {
                let temporary_name = self.fresh_temporary_name("binding_value");
                statements.push(Statement::Let {
                    name: temporary_name.clone(),
                    mutable: true,
                    value,
                });
                let mut then_branch = Vec::new();
                self.lower_for_of_pattern_binding(
                    &assign.left,
                    Expression::Identifier(temporary_name.clone()),
                    binding_kind,
                    &mut then_branch,
                )?;
                let mut else_branch = Vec::new();
                let default_value = self.lower_expression_with_name_hint(
                    &assign.right,
                    pattern_name_hint(&assign.left),
                )?;
                self.lower_for_of_pattern_binding(
                    &assign.left,
                    default_value,
                    binding_kind,
                    &mut else_branch,
                )?;
                statements.push(Statement::If {
                    condition: Expression::Binary {
                        op: BinaryOp::NotEqual,
                        left: Box::new(Expression::Identifier(temporary_name)),
                        right: Box::new(Expression::Undefined),
                    },
                    then_branch,
                    else_branch,
                });
            }
            Pat::Array(array) => {
                let has_rest = array
                    .elems
                    .iter()
                    .flatten()
                    .any(|element| matches!(element, Pat::Rest(_)));
                if !has_rest {
                    let pure_elision_count = self.pure_array_pattern_elision_count(array);
                    let iterator_name = self.fresh_temporary_name("array_iter");
                    let iterator_done_name = self.fresh_temporary_name("array_iter_done");
                    statements.push(Statement::Let {
                        name: iterator_name.clone(),
                        mutable: true,
                        value: Expression::GetIterator(Box::new(value.clone())),
                    });
                    statements.push(Statement::Let {
                        name: iterator_done_name.clone(),
                        mutable: true,
                        value: Expression::Bool(false),
                    });

                    if array.elems.is_empty() && pure_elision_count > 0 {
                        for _ in 0..pure_elision_count {
                            let step_name = self.fresh_temporary_name("array_step");
                            statements.push(Statement::Let {
                                name: step_name.clone(),
                                mutable: true,
                                value: Expression::Call {
                                    callee: Box::new(Expression::Member {
                                        object: Box::new(Expression::Identifier(
                                            iterator_name.clone(),
                                        )),
                                        property: Box::new(Expression::String("next".to_string())),
                                    }),
                                    arguments: Vec::new(),
                                },
                            });
                            statements.push(Statement::Assign {
                                name: iterator_done_name.clone(),
                                value: Expression::Member {
                                    object: Box::new(Expression::Identifier(step_name)),
                                    property: Box::new(Expression::String("done".to_string())),
                                },
                            });
                        }
                        statements.push(Statement::If {
                            condition: Expression::Binary {
                                op: BinaryOp::Equal,
                                left: Box::new(Expression::Identifier(iterator_done_name)),
                                right: Box::new(Expression::Bool(false)),
                            },
                            then_branch: vec![Statement::Expression(Expression::IteratorClose(
                                Box::new(Expression::Identifier(iterator_name)),
                            ))],
                            else_branch: Vec::new(),
                        });
                        return Ok(());
                    }

                    for element in &array.elems {
                        let step_name = self.fresh_temporary_name("array_step");
                        statements.push(Statement::Let {
                            name: step_name.clone(),
                            mutable: true,
                            value: Expression::Call {
                                callee: Box::new(Expression::Member {
                                    object: Box::new(Expression::Identifier(iterator_name.clone())),
                                    property: Box::new(Expression::String("next".to_string())),
                                }),
                                arguments: Vec::new(),
                            },
                        });
                        let step_done = Expression::Member {
                            object: Box::new(Expression::Identifier(step_name.clone())),
                            property: Box::new(Expression::String("done".to_string())),
                        };
                        statements.push(Statement::Assign {
                            name: iterator_done_name.clone(),
                            value: step_done.clone(),
                        });
                        let step_value = Expression::Conditional {
                            condition: Box::new(Expression::Binary {
                                op: BinaryOp::Equal,
                                left: Box::new(step_done),
                                right: Box::new(Expression::Bool(false)),
                            }),
                            then_expression: Box::new(Expression::Member {
                                object: Box::new(Expression::Identifier(step_name)),
                                property: Box::new(Expression::String("value".to_string())),
                            }),
                            else_expression: Box::new(Expression::Undefined),
                        };

                        if let Some(element) = element {
                            self.lower_for_of_pattern_binding(
                                element,
                                step_value,
                                binding_kind,
                                statements,
                            )?;
                        }
                    }

                    statements.push(Statement::If {
                        condition: Expression::Binary {
                            op: BinaryOp::Equal,
                            left: Box::new(Expression::Identifier(iterator_done_name)),
                            right: Box::new(Expression::Bool(false)),
                        },
                        then_branch: vec![Statement::Expression(Expression::IteratorClose(
                            Box::new(Expression::Identifier(iterator_name)),
                        ))],
                        else_branch: Vec::new(),
                    });
                    return Ok(());
                }

                let iterator_name = self.fresh_temporary_name("array_iter");
                let iterator_done_name = self.fresh_temporary_name("array_iter_done");
                statements.push(Statement::Let {
                    name: iterator_name.clone(),
                    mutable: true,
                    value: Expression::GetIterator(Box::new(value.clone())),
                });
                statements.push(Statement::Let {
                    name: iterator_done_name.clone(),
                    mutable: true,
                    value: Expression::Bool(false),
                });

                for element in &array.elems {
                    if let Some(Pat::Rest(rest)) = element {
                        let rest_array_name = self.fresh_temporary_name("array_rest");
                        let rest_step_name = self.fresh_temporary_name("array_step");
                        statements.push(Statement::Let {
                            name: rest_array_name.clone(),
                            mutable: true,
                            value: Expression::Array(Vec::new()),
                        });
                        statements.push(Statement::While {
                            labels: Vec::new(),
                            condition: Expression::Binary {
                                op: BinaryOp::Equal,
                                left: Box::new(Expression::Identifier(iterator_done_name.clone())),
                                right: Box::new(Expression::Bool(false)),
                            },
                            break_hook: None,
                            body: vec![
                                Statement::Let {
                                    name: rest_step_name.clone(),
                                    mutable: true,
                                    value: Expression::Call {
                                        callee: Box::new(Expression::Member {
                                            object: Box::new(Expression::Identifier(
                                                iterator_name.clone(),
                                            )),
                                            property: Box::new(Expression::String(
                                                "next".to_string(),
                                            )),
                                        }),
                                        arguments: Vec::new(),
                                    },
                                },
                                Statement::Assign {
                                    name: iterator_done_name.clone(),
                                    value: Expression::Member {
                                        object: Box::new(Expression::Identifier(
                                            rest_step_name.clone(),
                                        )),
                                        property: Box::new(Expression::String("done".to_string())),
                                    },
                                },
                                Statement::If {
                                    condition: Expression::Binary {
                                        op: BinaryOp::Equal,
                                        left: Box::new(Expression::Identifier(
                                            iterator_done_name.clone(),
                                        )),
                                        right: Box::new(Expression::Bool(false)),
                                    },
                                    then_branch: vec![Statement::Expression(Expression::Call {
                                        callee: Box::new(Expression::Member {
                                            object: Box::new(Expression::Identifier(
                                                rest_array_name.clone(),
                                            )),
                                            property: Box::new(Expression::String(
                                                "push".to_string(),
                                            )),
                                        }),
                                        arguments: vec![CallArgument::Expression(
                                            Expression::Member {
                                                object: Box::new(Expression::Identifier(
                                                    rest_step_name.clone(),
                                                )),
                                                property: Box::new(Expression::String(
                                                    "value".to_string(),
                                                )),
                                            },
                                        )],
                                    })],
                                    else_branch: Vec::new(),
                                },
                            ],
                        });
                        self.lower_for_of_pattern_binding(
                            &rest.arg,
                            Expression::Identifier(rest_array_name),
                            binding_kind,
                            statements,
                        )?;
                        break;
                    }

                    let step_name = self.fresh_temporary_name("array_step");
                    statements.push(Statement::Let {
                        name: step_name.clone(),
                        mutable: true,
                        value: Expression::Call {
                            callee: Box::new(Expression::Member {
                                object: Box::new(Expression::Identifier(iterator_name.clone())),
                                property: Box::new(Expression::String("next".to_string())),
                            }),
                            arguments: Vec::new(),
                        },
                    });
                    let step_done = Expression::Member {
                        object: Box::new(Expression::Identifier(step_name.clone())),
                        property: Box::new(Expression::String("done".to_string())),
                    };
                    statements.push(Statement::Assign {
                        name: iterator_done_name.clone(),
                        value: step_done.clone(),
                    });
                    let step_value = Expression::Conditional {
                        condition: Box::new(Expression::Binary {
                            op: BinaryOp::Equal,
                            left: Box::new(step_done),
                            right: Box::new(Expression::Bool(false)),
                        }),
                        then_expression: Box::new(Expression::Member {
                            object: Box::new(Expression::Identifier(step_name)),
                            property: Box::new(Expression::String("value".to_string())),
                        }),
                        else_expression: Box::new(Expression::Undefined),
                    };

                    if let Some(element) = element {
                        self.lower_for_of_pattern_binding(
                            element,
                            step_value,
                            binding_kind,
                            statements,
                        )?;
                    }
                }

                statements.push(Statement::If {
                    condition: Expression::Binary {
                        op: BinaryOp::Equal,
                        left: Box::new(Expression::Identifier(iterator_done_name)),
                        right: Box::new(Expression::Bool(false)),
                    },
                    then_branch: vec![Statement::Expression(Expression::IteratorClose(Box::new(
                        Expression::Identifier(iterator_name),
                    )))],
                    else_branch: Vec::new(),
                });
            }
            Pat::Object(object) => {
                self.emit_require_object_coercible_check(&value, statements);
                for property in &object.props {
                    match property {
                        ObjectPatProp::KeyValue(property) => {
                            let property_value = Expression::Member {
                                object: Box::new(value.clone()),
                                property: Box::new(self.lower_prop_name(&property.key)?),
                            };
                            self.lower_for_of_pattern_binding(
                                &property.value,
                                property_value,
                                binding_kind,
                                statements,
                            )?;
                        }
                        ObjectPatProp::Assign(property) => {
                            let binding_name =
                                self.resolve_binding_name(property.key.id.sym.as_ref());
                            let property_value = Expression::Member {
                                object: Box::new(value.clone()),
                                property: Box::new(Expression::String(
                                    property.key.id.sym.to_string(),
                                )),
                            };
                            let property_value = if let Some(default) = &property.value {
                                let default_value = self.lower_expression_with_name_hint(
                                    default,
                                    Some(binding_name.as_str()),
                                )?;
                                Expression::Conditional {
                                    condition: Box::new(Expression::Binary {
                                        op: BinaryOp::NotEqual,
                                        left: Box::new(property_value.clone()),
                                        right: Box::new(Expression::Undefined),
                                    }),
                                    then_expression: Box::new(property_value),
                                    else_expression: Box::new(default_value),
                                }
                            } else {
                                property_value
                            };
                            statements.push(match binding_kind {
                                ForOfPatternBindingKind::Var => Statement::Var {
                                    name: binding_name,
                                    value: property_value,
                                },
                                ForOfPatternBindingKind::Assignment => Statement::Assign {
                                    name: binding_name,
                                    value: property_value,
                                },
                                ForOfPatternBindingKind::Lexical { mutable } => Statement::Let {
                                    name: binding_name,
                                    mutable,
                                    value: property_value,
                                },
                            });
                        }
                        ObjectPatProp::Rest(_) => bail!("unsupported for-of binding pattern"),
                    }
                }
            }
            _ => bail!("unsupported for-of binding pattern"),
        }

        Ok(())
    }

    pub(crate) fn emit_require_object_coercible_check(
        &mut self,
        value: &Expression,
        statements: &mut Vec<Statement>,
    ) {
        let is_nullish = Expression::Binary {
            op: BinaryOp::LogicalOr,
            left: Box::new(Expression::Binary {
                op: BinaryOp::Equal,
                left: Box::new(value.clone()),
                right: Box::new(Expression::Null),
            }),
            right: Box::new(Expression::Binary {
                op: BinaryOp::Equal,
                left: Box::new(value.clone()),
                right: Box::new(Expression::Undefined),
            }),
        };

        statements.push(Statement::If {
            condition: is_nullish,
            then_branch: vec![Statement::Throw(Expression::New {
                callee: Box::new(Expression::Identifier("TypeError".to_string())),
                arguments: Vec::new(),
            })],
            else_branch: Vec::new(),
        });
    }

    pub(super) fn lower_for_of_expression_target(
        &mut self,
        expression: &Expr,
    ) -> Result<AssignmentTarget> {
        match expression {
            Expr::Ident(identifier) => Ok(AssignmentTarget::Identifier(
                self.resolve_binding_name(identifier.sym.as_ref()),
            )),
            Expr::Member(member) => Ok(AssignmentTarget::Member {
                object: self.lower_expression(&member.obj)?,
                property: self.lower_member_property(&member.prop)?,
            }),
            Expr::Paren(parenthesized) => self.lower_for_of_expression_target(&parenthesized.expr),
            _ => bail!("unsupported for-of assignment target"),
        }
    }

    pub(super) fn lower_assignment_target(
        &mut self,
        target: &AssignTarget,
    ) -> Result<AssignmentTarget> {
        match target {
            AssignTarget::Simple(SimpleAssignTarget::Ident(identifier)) => Ok(
                AssignmentTarget::Identifier(self.resolve_binding_name(identifier.id.sym.as_ref())),
            ),
            AssignTarget::Simple(SimpleAssignTarget::Member(member)) => {
                Ok(AssignmentTarget::Member {
                    object: self.lower_expression(&member.obj)?,
                    property: self.lower_member_property(&member.prop)?,
                })
            }
            AssignTarget::Simple(SimpleAssignTarget::SuperProp(super_property)) => {
                Ok(AssignmentTarget::SuperMember {
                    property: self.lower_super_property(super_property)?,
                })
            }
            _ => bail!("unsupported assignment target"),
        }
    }
}
