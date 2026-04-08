use super::*;

impl Lowerer {
    pub(crate) fn lower_class_declaration(
        &mut self,
        class_declaration: &ClassDecl,
    ) -> Result<Vec<Statement>> {
        Ok(vec![Statement::Declaration {
            body: self.lower_class_definition_with_mode(
                &class_declaration.class,
                self.resolve_binding_name(class_declaration.ident.sym.as_ref()),
                false,
            )?,
        }])
    }

    pub(crate) fn lower_generator_class_declaration(
        &mut self,
        class_declaration: &ClassDecl,
    ) -> Result<Vec<Statement>> {
        Ok(vec![Statement::Declaration {
            body: self.lower_class_definition_with_mode(
                &class_declaration.class,
                self.resolve_binding_name(class_declaration.ident.sym.as_ref()),
                true,
            )?,
        }])
    }

    pub(crate) fn lower_class_expression(
        &mut self,
        class_expression: &swc_ecma_ast::ClassExpr,
        name_hint: Option<&str>,
    ) -> Result<Expression> {
        let explicit_name = class_expression
            .ident
            .as_ref()
            .map(|identifier| identifier.sym.to_string());
        let pushed_scope = explicit_name.is_some();
        if let Some(explicit_name) = explicit_name.as_ref() {
            self.push_binding_scope(vec![explicit_name.clone()]);
            let scoped_name = self.fresh_scoped_binding_name(explicit_name);
            if let Some(scope) = self.binding_scopes.last_mut() {
                scope.renames.insert(explicit_name.clone(), scoped_name);
            }
        }
        let class_name = explicit_name
            .as_ref()
            .map(|name| self.resolve_binding_name(name))
            .unwrap_or_else(|| self.fresh_temporary_name("class_expr"));
        let display_name = explicit_name
            .or_else(|| name_hint.map(str::to_string))
            .unwrap_or_default();
        let init_name = self.fresh_temporary_name("class_init");
        let init_body_result = self.lower_class_definition_with_mode(
            &class_expression.class,
            class_name.clone(),
            false,
        );
        if pushed_scope {
            self.pop_binding_scope();
        }
        let mut init_body = init_body_result?;
        init_body.push(define_property_statement(
            Expression::Identifier(class_name.clone()),
            Expression::String("name".to_string()),
            data_property_descriptor(Expression::String(display_name), false, false, true),
        ));
        init_body.push(Statement::Return(Expression::Identifier(class_name)));

        self.functions.push(FunctionDeclaration {
            name: init_name.clone(),
            top_level_binding: None,
            params: Vec::new(),
            body: init_body,
            register_global: false,
            kind: FunctionKind::Ordinary,
            self_binding: None,
            mapped_arguments: false,
            strict: true,
            lexical_this: false,
            derived_constructor: false,
            length: 0,
        });

        Ok(Expression::Call {
            callee: Box::new(Expression::Identifier(init_name)),
            arguments: Vec::new(),
        })
    }

    pub(crate) fn lower_generator_class_expression(
        &mut self,
        class_expression: &swc_ecma_ast::ClassExpr,
        name_hint: Option<&str>,
    ) -> Result<(Vec<Statement>, Expression)> {
        let explicit_name = class_expression
            .ident
            .as_ref()
            .map(|identifier| identifier.sym.to_string());
        let pushed_scope = explicit_name.is_some();
        if let Some(explicit_name) = explicit_name.as_ref() {
            self.push_binding_scope(vec![explicit_name.clone()]);
            let scoped_name = self.fresh_scoped_binding_name(explicit_name);
            if let Some(scope) = self.binding_scopes.last_mut() {
                scope.renames.insert(explicit_name.clone(), scoped_name);
            }
        }
        let class_name = explicit_name
            .as_ref()
            .map(|name| self.resolve_binding_name(name))
            .unwrap_or_else(|| self.fresh_temporary_name("class_expr"));
        let display_name = explicit_name
            .or_else(|| name_hint.map(str::to_string))
            .unwrap_or_default();

        let statements_result = self.lower_class_definition_with_mode(
            &class_expression.class,
            class_name.clone(),
            true,
        );
        if pushed_scope {
            self.pop_binding_scope();
        }
        let mut statements = statements_result?;
        statements.push(define_property_statement(
            Expression::Identifier(class_name.clone()),
            Expression::String("name".to_string()),
            data_property_descriptor(Expression::String(display_name), false, false, true),
        ));

        Ok((statements, Expression::Identifier(class_name)))
    }

    pub(crate) fn lower_class_definition_with_mode(
        &mut self,
        class: &Class,
        binding_name: String,
        generator_body: bool,
    ) -> Result<Vec<Statement>> {
        self.private_name_scopes
            .push(self.class_private_name_map(class, &binding_name));
        let class_identifier = Expression::Identifier(binding_name.clone());
        let extends_null = matches!(class.super_class.as_deref(), Some(Expr::Lit(Lit::Null(_))));
        let super_name = class
            .super_class
            .as_ref()
            .filter(|_| !extends_null)
            .map(|_| self.fresh_temporary_name("class_super"));
        let constructor_name =
            self.lower_class_constructor(class, &binding_name, super_name.as_deref())?;
        let prototype_parent = if extends_null {
            Expression::Null
        } else {
            super_name
                .as_ref()
                .map(|name| Expression::Member {
                    object: Box::new(Expression::Identifier(name.clone())),
                    property: Box::new(Expression::String("prototype".to_string())),
                })
                .unwrap_or(Expression::Member {
                    object: Box::new(Expression::Identifier("Object".to_string())),
                    property: Box::new(Expression::String("prototype".to_string())),
                })
        };
        let prototype_target = Expression::Member {
            object: Box::new(class_identifier.clone()),
            property: Box::new(Expression::String("prototype".to_string())),
        };

        let mut statements = Vec::new();
        let mut instance_public_field_initializers = Vec::new();
        if let (Some(super_expression), Some(super_name)) =
            (&class.super_class, super_name.as_ref())
        {
            statements.push(Statement::Let {
                name: super_name.clone(),
                mutable: false,
                value: self.lower_expression(super_expression)?,
            });
        }

        statements.extend([
            Statement::Let {
                name: binding_name.clone(),
                mutable: true,
                value: Expression::Identifier(constructor_name.clone()),
            },
            define_property_statement(
                class_identifier.clone(),
                Expression::String("name".to_string()),
                data_property_descriptor(
                    Expression::String(binding_name.clone()),
                    false,
                    false,
                    true,
                ),
            ),
            Statement::AssignMember {
                object: class_identifier.clone(),
                property: Expression::String("prototype".to_string()),
                value: Expression::Call {
                    callee: Box::new(Expression::Member {
                        object: Box::new(Expression::Identifier("Object".to_string())),
                        property: Box::new(Expression::String("create".to_string())),
                    }),
                    arguments: vec![CallArgument::Expression(prototype_parent)],
                },
            },
            define_property_statement(
                prototype_target.clone(),
                Expression::String("constructor".to_string()),
                data_property_descriptor(class_identifier.clone(), true, false, true),
            ),
        ]);
        if let Some(super_name) = super_name.as_ref() {
            statements.push(Statement::Expression(Expression::Call {
                callee: Box::new(Expression::Member {
                    object: Box::new(Expression::Identifier("Object".to_string())),
                    property: Box::new(Expression::String("setPrototypeOf".to_string())),
                }),
                arguments: vec![
                    CallArgument::Expression(class_identifier.clone()),
                    CallArgument::Expression(Expression::Identifier(super_name.clone())),
                ],
            }));
        }

        for member in &class.body {
            match member {
                ClassMember::ClassProp(property) => {
                    let (mut property_prefix, lowered_property_name) =
                        self.lower_class_prop_name(&property.key, generator_body)?;
                    statements.append(&mut property_prefix);
                    let property_name = match &property.key {
                        PropName::Computed(_) => {
                            let computed_name = self.fresh_temporary_name("class_field_name");
                            statements.push(Statement::Let {
                                name: computed_name.clone(),
                                mutable: false,
                                value: lowered_property_name,
                            });
                            Expression::Identifier(computed_name)
                        }
                        _ => lowered_property_name,
                    };
                    let value = property
                        .value
                        .as_ref()
                        .map(|value| self.lower_expression(value))
                        .transpose()?
                        .unwrap_or(Expression::Undefined);
                    if property.is_static {
                        statements.push(Statement::AssignMember {
                            object: class_identifier.clone(),
                            property: property_name,
                            value,
                        });
                    } else {
                        instance_public_field_initializers.push(Statement::AssignMember {
                            object: Expression::This,
                            property: property_name,
                            value,
                        });
                    }
                }
                _ => {
                    statements.extend(self.lower_class_member_with_mode(
                        member,
                        &binding_name,
                        &prototype_target,
                        generator_body,
                    )?);
                }
            }
        }

        if !instance_public_field_initializers.is_empty() {
            let constructor = self
                .functions
                .iter_mut()
                .rfind(|function| function.name == constructor_name)
                .context(
                    "lowered class constructor should exist for public field initialization",
                )?;
            constructor
                .body
                .splice(0..0, instance_public_field_initializers);
        }

        for member in &class.body {
            if let ClassMember::PrivateProp(property) = member {
                if !property.is_static {
                    continue;
                }
                let value = property
                    .value
                    .as_ref()
                    .map(|value| self.lower_expression(value))
                    .transpose()?
                    .unwrap_or(Expression::Undefined);
                statements.push(Statement::AssignMember {
                    object: class_identifier.clone(),
                    property: self.lower_private_name(&property.key)?,
                    value,
                });
            }
        }

        self.private_name_scopes.pop();

        Ok(statements)
    }

    pub(crate) fn lower_class_constructor(
        &mut self,
        class: &Class,
        binding_name: &str,
        super_name: Option<&str>,
    ) -> Result<String> {
        let constructor = class.body.iter().find_map(|member| match member {
            ClassMember::Constructor(constructor) => Some(constructor),
            _ => None,
        });

        let generated_name = format!(
            "__ayy_class_ctor_{}__name_{}",
            self.fresh_temporary_name("ctor"),
            binding_name
        );

        let (params, param_setup, body, length) = if let Some(constructor) = constructor {
            let (params, param_setup, length) = lower_constructor_parameters(self, constructor)?;
            let body = if let Some(body) = &constructor.body {
                self.constructor_super_stack
                    .push(super_name.map(ToOwned::to_owned));
                self.strict_modes.push(true);
                let lowered = self.lower_statements(&body.stmts, true, false);
                self.strict_modes.pop();
                self.constructor_super_stack.pop();
                lowered?
            } else {
                Vec::new()
            };
            (params, param_setup, body, length)
        } else {
            (Vec::new(), Vec::new(), Vec::new(), 0)
        };

        let mut body = body;
        body.insert(
            0,
            Statement::If {
                condition: Expression::Binary {
                    op: BinaryOp::Equal,
                    left: Box::new(Expression::NewTarget),
                    right: Box::new(Expression::Undefined),
                },
                then_branch: vec![Statement::Throw(Expression::New {
                    callee: Box::new(Expression::Identifier("TypeError".to_string())),
                    arguments: Vec::new(),
                })],
                else_branch: Vec::new(),
            },
        );
        for member in class.body.iter().rev() {
            if let ClassMember::PrivateProp(property) = member {
                if property.is_static {
                    continue;
                }
                let value = property
                    .value
                    .as_ref()
                    .map(|value| self.lower_expression(value))
                    .transpose()?
                    .unwrap_or(Expression::Undefined);
                body.insert(
                    0,
                    Statement::AssignMember {
                        object: Expression::This,
                        property: self.lower_private_name(&property.key)?,
                        value,
                    },
                );
            }
        }
        body.splice(0..0, param_setup);

        self.functions.push(FunctionDeclaration {
            name: generated_name.clone(),
            top_level_binding: None,
            params,
            body,
            register_global: false,
            kind: FunctionKind::Ordinary,
            self_binding: Some(binding_name.to_string()),
            mapped_arguments: false,
            strict: true,
            lexical_this: false,
            derived_constructor: super_name.is_some(),
            length,
        });

        Ok(generated_name)
    }

    pub(crate) fn lower_class_member_with_mode(
        &mut self,
        member: &ClassMember,
        class_name: &str,
        prototype_target: &Expression,
        generator_body: bool,
    ) -> Result<Vec<Statement>> {
        match member {
            ClassMember::Constructor(_) | ClassMember::Empty(_) | ClassMember::PrivateProp(_) => {
                Ok(Vec::new())
            }
            ClassMember::StaticBlock(block) => {
                self.strict_modes.push(true);
                let lowered = self.lower_statements(&block.body.stmts, false, false);
                self.strict_modes.pop();
                lowered
            }
            ClassMember::Method(method) => {
                let (mut prefix, property) =
                    self.lower_class_prop_name(&method.key, generator_body)?;
                let target = if method.is_static {
                    Expression::Identifier(class_name.to_string())
                } else {
                    prototype_target.clone()
                };
                if method.kind == MethodKind::Getter {
                    if let Some(private_alias) =
                        self.lower_private_method_alias_getter(method, &target)?
                    {
                        prefix.push(define_property_statement(
                            target,
                            property,
                            data_property_descriptor(private_alias, false, false, true),
                        ));
                        return Ok(prefix);
                    }
                }
                prefix.extend(self.lower_defined_class_method(
                    class_name,
                    prototype_target,
                    method.is_static,
                    method.kind,
                    property,
                    &method.function,
                )?);
                Ok(prefix)
            }
            ClassMember::PrivateMethod(method) => {
                let property = self.lower_private_name(&method.key)?;
                self.lower_defined_class_method(
                    class_name,
                    prototype_target,
                    method.is_static,
                    method.kind,
                    property,
                    &method.function,
                )
            }
            other => bail!("unsupported class member: {other:?}"),
        }
    }

    fn lower_class_prop_name(
        &mut self,
        name: &PropName,
        generator_body: bool,
    ) -> Result<(Vec<Statement>, Expression)> {
        if !generator_body {
            return Ok((Vec::new(), self.lower_prop_name(name)?));
        }

        Ok(match name {
            PropName::Ident(identifier) => {
                (Vec::new(), Expression::String(identifier.sym.to_string()))
            }
            PropName::Str(string) => (
                Vec::new(),
                Expression::String(string.value.to_string_lossy().into_owned()),
            ),
            PropName::Num(number) => (Vec::new(), Expression::Number(number.value)),
            PropName::Computed(computed) => {
                if let Some((prefix, value)) =
                    self.lower_generator_assignment_value(&computed.expr)?
                {
                    (prefix, value)
                } else {
                    (Vec::new(), self.lower_expression(&computed.expr)?)
                }
            }
            _ => bail!("unsupported object property key"),
        })
    }

    pub(crate) fn lower_private_method_alias_getter(
        &mut self,
        method: &ClassMethod,
        target: &Expression,
    ) -> Result<Option<Expression>> {
        let Some(body) = method.function.body.as_ref() else {
            return Ok(None);
        };
        if !method.function.params.is_empty() || body.stmts.len() != 1 {
            return Ok(None);
        }
        let swc_ecma_ast::Stmt::Return(return_statement) = &body.stmts[0] else {
            return Ok(None);
        };
        let Some(return_value) = return_statement.arg.as_deref() else {
            return Ok(None);
        };
        let Expr::Member(member) = return_value else {
            return Ok(None);
        };
        if !matches!(member.obj.as_ref(), Expr::This(_)) {
            return Ok(None);
        }
        let MemberProp::PrivateName(private_name) = &member.prop else {
            return Ok(None);
        };
        Ok(Some(Expression::Member {
            object: Box::new(target.clone()),
            property: Box::new(self.lower_private_name(private_name)?),
        }))
    }

    pub(crate) fn lower_defined_class_method(
        &mut self,
        class_name: &str,
        prototype_target: &Expression,
        is_static: bool,
        kind: MethodKind,
        property: Expression,
        function: &Function,
    ) -> Result<Vec<Statement>> {
        let target = if is_static {
            Expression::Identifier(class_name.to_string())
        } else {
            prototype_target.clone()
        };
        let descriptor = match kind {
            MethodKind::Method => {
                let method_name = self.lower_class_method_function(function)?;
                data_property_descriptor(Expression::Identifier(method_name), true, false, true)
            }
            MethodKind::Getter => {
                let getter_name = self.lower_class_method_function(function)?;
                getter_property_descriptor(Expression::Identifier(getter_name), false, true)
            }
            MethodKind::Setter => {
                let setter_name = self.lower_class_method_function(function)?;
                setter_property_descriptor(Expression::Identifier(setter_name), false, true)
            }
        };

        if is_static {
            return Ok(self.lower_static_class_method_definition(target, property, descriptor));
        }

        Ok(vec![define_property_statement(
            target, property, descriptor,
        )])
    }

    pub(crate) fn lower_static_class_method_definition(
        &mut self,
        target: Expression,
        property: Expression,
        descriptor: Expression,
    ) -> Vec<Statement> {
        if matches!(&property, Expression::String(name) if name == "prototype") {
            return vec![Statement::Throw(Expression::New {
                callee: Box::new(Expression::Identifier("TypeError".to_string())),
                arguments: Vec::new(),
            })];
        }

        if matches!(
            property,
            Expression::String(_)
                | Expression::Number(_)
                | Expression::BigInt(_)
                | Expression::Bool(_)
                | Expression::Null
                | Expression::Undefined
        ) {
            return vec![define_property_statement(target, property, descriptor)];
        }

        let property_name = self.fresh_temporary_name("class_prop");
        let property_identifier = Expression::Identifier(property_name.clone());

        vec![
            Statement::Let {
                name: property_name,
                mutable: false,
                value: property,
            },
            Statement::If {
                condition: Expression::Binary {
                    op: BinaryOp::Equal,
                    left: Box::new(property_identifier.clone()),
                    right: Box::new(Expression::String("prototype".to_string())),
                },
                then_branch: vec![Statement::Throw(Expression::New {
                    callee: Box::new(Expression::Identifier("TypeError".to_string())),
                    arguments: Vec::new(),
                })],
                else_branch: vec![define_property_statement(
                    target,
                    property_identifier,
                    descriptor,
                )],
            },
        ]
    }

    pub(crate) fn lower_class_method_function(&mut self, function: &Function) -> Result<String> {
        self.next_function_expression_id += 1;
        let generated_name = format!("__ayy_class_method_{}", self.next_function_expression_id);
        self.strict_modes.push(true);
        let (params, body) = self.lower_function_parts(function, &[])?;
        self.strict_modes.pop();

        self.functions.push(FunctionDeclaration {
            name: generated_name.clone(),
            top_level_binding: None,
            params,
            body,
            register_global: false,
            kind: lower_function_kind(function.is_generator, function.is_async),
            self_binding: None,
            mapped_arguments: false,
            strict: true,
            lexical_this: false,
            derived_constructor: false,
            length: expected_argument_count(function.params.iter().map(|parameter| &parameter.pat)),
        });

        Ok(generated_name)
    }
}
