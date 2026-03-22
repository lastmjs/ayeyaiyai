use super::*;

impl Lowerer {
    pub(crate) fn lower_expression(&mut self, expression: &Expr) -> Result<Expression> {
        self.lower_expression_with_name_hint(expression, None)
    }

    pub(crate) fn lower_expression_with_name_hint(
        &mut self,
        expression: &Expr,
        name_hint: Option<&str>,
    ) -> Result<Expression> {
        if let Some(arguments) = console_log_arguments(expression) {
            return Ok(Expression::Call {
                callee: Box::new(Expression::Identifier("__ayyPrint".to_string())),
                arguments: arguments
                    .iter()
                    .map(|argument| {
                        let expression = self.lower_expression(&argument.expr)?;
                        Ok(if argument.spread.is_some() {
                            CallArgument::Spread(expression)
                        } else {
                            CallArgument::Expression(expression)
                        })
                    })
                    .collect::<Result<Vec<_>>>()?,
            });
        }

        match expression {
            Expr::Lit(Lit::Num(number)) => Ok(Expression::Number(number.value)),
            Expr::Lit(Lit::BigInt(bigint)) => Ok(Expression::BigInt(parse_bigint_literal(
                &bigint.value.to_string(),
            )?)),
            Expr::Lit(Lit::Str(string)) => Ok(Expression::String(
                string.value.to_string_lossy().into_owned(),
            )),
            Expr::Lit(Lit::Bool(boolean)) => Ok(Expression::Bool(boolean.value)),
            Expr::Lit(Lit::Null(_)) => Ok(Expression::Null),
            Expr::MetaProp(meta_property) => match meta_property.kind {
                MetaPropKind::NewTarget => Ok(Expression::NewTarget),
                _ => bail!("unsupported expression: {expression:?}"),
            },
            Expr::Lit(Lit::Regex(regex)) => Ok(Expression::Call {
                callee: Box::new(Expression::Identifier("RegExp".to_string())),
                arguments: vec![
                    CallArgument::Expression(Expression::String(regex.exp.to_string())),
                    CallArgument::Expression(Expression::String(regex.flags.to_string())),
                ],
            }),
            Expr::Tpl(template) => self.lower_template_expression(template),
            Expr::Array(array) => Ok(Expression::Array(
                array
                    .elems
                    .iter()
                    .map(|element| match element {
                        Some(element) => {
                            let expression = self.lower_expression(&element.expr)?;
                            Ok(if element.spread.is_some() {
                                ArrayElement::Spread(expression)
                            } else {
                                ArrayElement::Expression(expression)
                            })
                        }
                        None => Ok(ArrayElement::Expression(Expression::Undefined)),
                    })
                    .collect::<Result<Vec<_>>>()?,
            )),
            Expr::Object(object) => Ok(Expression::Object(
                object
                    .props
                    .iter()
                    .map(|property| self.lower_object_entry(property))
                    .collect::<Result<Vec<_>>>()?,
            )),
            Expr::Ident(identifier) => Ok(Expression::Identifier(
                self.resolve_binding_name(identifier.sym.as_ref()),
            )),
            Expr::This(_) => Ok(Expression::This),
            Expr::Member(member) => Ok(Expression::Member {
                object: Box::new(self.lower_expression(&member.obj)?),
                property: Box::new(self.lower_member_property(&member.prop)?),
            }),
            Expr::SuperProp(super_property) => Ok(Expression::SuperMember {
                property: Box::new(self.lower_super_property(super_property)?),
            }),
            Expr::Paren(parenthesized) => {
                self.lower_expression_with_name_hint(&parenthesized.expr, name_hint)
            }
            Expr::Await(await_expression) => Ok(Expression::Await(Box::new(
                self.lower_expression_with_name_hint(&await_expression.arg, name_hint)?,
            ))),
            Expr::Unary(unary) => Ok(Expression::Unary {
                op: lower_unary_operator(unary.op)?,
                expression: Box::new(self.lower_expression(&unary.arg)?),
            }),
            Expr::Bin(binary) => Ok(Expression::Binary {
                op: lower_binary_operator(binary.op)?,
                left: Box::new(self.lower_expression(&binary.left)?),
                right: Box::new(self.lower_expression(&binary.right)?),
            }),
            Expr::Cond(conditional) => Ok(Expression::Conditional {
                condition: Box::new(self.lower_expression(&conditional.test)?),
                then_expression: Box::new(self.lower_expression(&conditional.cons)?),
                else_expression: Box::new(self.lower_expression(&conditional.alt)?),
            }),
            Expr::Seq(sequence) => Ok(Expression::Sequence(
                sequence
                    .exprs
                    .iter()
                    .map(|expression| self.lower_expression(expression))
                    .collect::<Result<Vec<_>>>()?,
            )),
            Expr::Assign(assignment) => {
                let target = self.lower_assignment_target(&assignment.left)?;
                let right = match &target {
                    AssignmentTarget::Identifier(name) => {
                        self.lower_expression_with_name_hint(&assignment.right, Some(name))?
                    }
                    AssignmentTarget::Member { .. } | AssignmentTarget::SuperMember { .. } => {
                        self.lower_expression(&assignment.right)?
                    }
                };

                match assignment.op {
                    AssignOp::Assign => self.lower_assignment_expression(target, right),
                    AssignOp::AndAssign => self.lower_logical_assignment_expression(
                        target,
                        right,
                        LogicalAssignmentKind::And,
                    ),
                    AssignOp::OrAssign => self.lower_logical_assignment_expression(
                        target,
                        right,
                        LogicalAssignmentKind::Or,
                    ),
                    AssignOp::NullishAssign => self.lower_logical_assignment_expression(
                        target,
                        right,
                        LogicalAssignmentKind::Nullish,
                    ),
                    operator => {
                        let binary_operator = lower_binary_operator(
                            operator
                                .to_update()
                                .context("unsupported assignment operator")?,
                        )?;
                        let value = match &target {
                            AssignmentTarget::Identifier(name) => Expression::Binary {
                                op: binary_operator,
                                left: Box::new(Expression::Identifier(name.clone())),
                                right: Box::new(right),
                            },
                            AssignmentTarget::Member { object, property } => Expression::Binary {
                                op: binary_operator,
                                left: Box::new(Expression::Member {
                                    object: Box::new(object.clone()),
                                    property: Box::new(property.clone()),
                                }),
                                right: Box::new(right),
                            },
                            AssignmentTarget::SuperMember { property } => Expression::Binary {
                                op: binary_operator,
                                left: Box::new(Expression::SuperMember {
                                    property: Box::new(property.clone()),
                                }),
                                right: Box::new(right),
                            },
                        };

                        self.lower_assignment_expression(target, value)
                    }
                }
            }
            Expr::Call(call) => match &call.callee {
                Callee::Expr(callee) => Ok(Expression::Call {
                    callee: Box::new(self.lower_expression(callee)?),
                    arguments: call
                        .args
                        .iter()
                        .map(|argument| {
                            let expression = self.lower_expression(&argument.expr)?;
                            Ok(if argument.spread.is_some() {
                                CallArgument::Spread(expression)
                            } else {
                                CallArgument::Expression(expression)
                            })
                        })
                        .collect::<Result<Vec<_>>>()?,
                }),
                Callee::Super(_) => {
                    let super_name = self
                        .constructor_super_stack
                        .last()
                        .and_then(|name| name.clone())
                        .context("`super()` is only supported in derived constructors")?;
                    Ok(Expression::SuperCall {
                        callee: Box::new(Expression::Identifier(super_name)),
                        arguments: call
                            .args
                            .iter()
                            .map(|argument| {
                                let expression = self.lower_expression(&argument.expr)?;
                                Ok(if argument.spread.is_some() {
                                    CallArgument::Spread(expression)
                                } else {
                                    CallArgument::Expression(expression)
                                })
                            })
                            .collect::<Result<Vec<_>>>()?,
                    })
                }
                Callee::Import(_) => self.lower_dynamic_import_expression(call),
            },
            Expr::TaggedTpl(tagged_template) => Ok(Expression::Call {
                callee: Box::new(self.lower_expression(&tagged_template.tag)?),
                arguments: std::iter::once(Ok(CallArgument::Expression(Expression::Array(
                    tagged_template
                        .tpl
                        .quasis
                        .iter()
                        .map(|quasi| {
                            Ok(ArrayElement::Expression(Expression::String(
                                quasi
                                    .cooked
                                    .as_ref()
                                    .and_then(|value| value.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                            )))
                        })
                        .collect::<Result<Vec<_>>>()?,
                ))))
                .chain(tagged_template.tpl.exprs.iter().map(|expression| {
                    self.lower_expression(expression)
                        .map(CallArgument::Expression)
                }))
                .collect::<Result<Vec<_>>>()?,
            }),
            Expr::New(new_expression) => Ok(Expression::New {
                callee: Box::new(self.lower_expression(&new_expression.callee)?),
                arguments: new_expression
                    .args
                    .as_deref()
                    .unwrap_or(&[])
                    .iter()
                    .map(|argument| {
                        let expression = self.lower_expression(&argument.expr)?;
                        Ok(if argument.spread.is_some() {
                            CallArgument::Spread(expression)
                        } else {
                            CallArgument::Expression(expression)
                        })
                    })
                    .collect::<Result<Vec<_>>>()?,
            }),
            Expr::Fn(function_expression) => {
                self.lower_function_expression(function_expression, name_hint)
            }
            Expr::Class(class_expression) => {
                self.lower_class_expression(class_expression, name_hint)
            }
            Expr::Arrow(arrow_expression) => {
                self.lower_arrow_expression(arrow_expression, name_hint)
            }
            Expr::Update(update) => {
                let name = match &*update.arg {
                    Expr::Ident(identifier) => self.resolve_binding_name(identifier.sym.as_ref()),
                    other => self
                        .try_lower_top_level_this_member_update(other)?
                        .context("only identifier update expressions are supported")?,
                };

                Ok(Expression::Update {
                    name,
                    op: lower_update_operator(update.op),
                    prefix: update.prefix,
                })
            }
            _ => bail!("unsupported expression: {expression:?}"),
        }
    }

    pub(super) fn lower_assignment_expression(
        &mut self,
        target: AssignmentTarget,
        value: Expression,
    ) -> Result<Expression> {
        Ok(target.into_expression(value))
    }

    pub(super) fn lower_logical_assignment_expression(
        &mut self,
        target: AssignmentTarget,
        right: Expression,
        kind: LogicalAssignmentKind,
    ) -> Result<Expression> {
        let current = target.as_expression();
        let assignment = self.lower_assignment_expression(target, right)?;

        let expression = match kind {
            LogicalAssignmentKind::And => Expression::Conditional {
                condition: Box::new(current.clone()),
                then_expression: Box::new(assignment),
                else_expression: Box::new(current),
            },
            LogicalAssignmentKind::Or => Expression::Conditional {
                condition: Box::new(current.clone()),
                then_expression: Box::new(current),
                else_expression: Box::new(assignment),
            },
            LogicalAssignmentKind::Nullish => {
                let not_undefined = Expression::Binary {
                    op: BinaryOp::NotEqual,
                    left: Box::new(current.clone()),
                    right: Box::new(Expression::Undefined),
                };
                let not_null = Expression::Binary {
                    op: BinaryOp::NotEqual,
                    left: Box::new(current.clone()),
                    right: Box::new(Expression::Null),
                };

                Expression::Conditional {
                    condition: Box::new(Expression::Binary {
                        op: BinaryOp::LogicalAnd,
                        left: Box::new(not_undefined),
                        right: Box::new(not_null),
                    }),
                    then_expression: Box::new(current),
                    else_expression: Box::new(assignment),
                }
            }
        };

        Ok(expression)
    }

    pub(crate) fn lower_object_entry(&mut self, property: &PropOrSpread) -> Result<ObjectEntry> {
        match property {
            PropOrSpread::Spread(spread) => {
                Ok(ObjectEntry::Spread(self.lower_expression(&spread.expr)?))
            }
            PropOrSpread::Prop(property) => match &**property {
                Prop::Shorthand(identifier) => Ok(ObjectEntry::Data {
                    key: Expression::String(identifier.sym.to_string()),
                    value: Expression::Identifier(identifier.sym.to_string()),
                }),
                Prop::Method(method) => {
                    self.next_function_expression_id += 1;
                    let generated_name =
                        format!("__ayy_method_{}", self.next_function_expression_id);
                    let (params, body) = self.lower_function_parts(&method.function, &[])?;

                    self.functions.push(FunctionDeclaration {
                        name: generated_name.clone(),
                        top_level_binding: None,
                        params,
                        body,
                        register_global: false,
                        kind: lower_function_kind(
                            method.function.is_generator,
                            method.function.is_async,
                        ),
                        self_binding: None,
                        mapped_arguments: self.function_has_mapped_arguments(&method.function),
                        strict: self.function_strict_mode(&method.function),
                        lexical_this: false,
                        length: expected_argument_count(
                            method
                                .function
                                .params
                                .iter()
                                .map(|parameter| &parameter.pat),
                        ),
                    });

                    Ok(ObjectEntry::Data {
                        key: self.lower_prop_name(&method.key)?,
                        value: Expression::Identifier(generated_name),
                    })
                }
                Prop::Getter(getter) => {
                    self.next_function_expression_id += 1;
                    let generated_name =
                        format!("__ayy_getter_{}", self.next_function_expression_id);
                    let body = getter.body.as_ref().context("getters must have a body")?;
                    let strict_mode =
                        self.current_strict_mode() || script_has_use_strict_directive(&body.stmts);
                    self.strict_modes.push(strict_mode);
                    let lowered_body = self.lower_statements(&body.stmts, true, false);
                    self.strict_modes.pop();
                    let lowered_body = lowered_body?;

                    self.functions.push(FunctionDeclaration {
                        name: generated_name.clone(),
                        top_level_binding: None,
                        params: Vec::new(),
                        body: lowered_body,
                        register_global: false,
                        kind: FunctionKind::Ordinary,
                        self_binding: None,
                        mapped_arguments: false,
                        strict: strict_mode,
                        lexical_this: false,
                        length: 0,
                    });

                    Ok(ObjectEntry::Getter {
                        key: self.lower_prop_name(&getter.key)?,
                        getter: Expression::Identifier(generated_name),
                    })
                }
                Prop::Setter(setter) => {
                    self.next_function_expression_id += 1;
                    let generated_name =
                        format!("__ayy_setter_{}", self.next_function_expression_id);
                    let body = setter.body.as_ref().context("setters must have a body")?;
                    let strict_mode =
                        self.current_strict_mode() || script_has_use_strict_directive(&body.stmts);
                    self.strict_modes.push(strict_mode);
                    let lowered = (|| -> Result<(Parameter, Vec<Statement>)> {
                        let (params, mut param_setup) = lower_parameter(self, &setter.param)?;
                        let mut lowered_body = self.lower_statements(&body.stmts, true, false)?;
                        lowered_body.splice(0..0, param_setup.drain(..));
                        Ok((params, lowered_body))
                    })();
                    self.strict_modes.pop();
                    let (params, lowered_body) = lowered?;

                    self.functions.push(FunctionDeclaration {
                        name: generated_name.clone(),
                        top_level_binding: None,
                        params: vec![params],
                        body: lowered_body,
                        register_global: false,
                        kind: FunctionKind::Ordinary,
                        self_binding: None,
                        mapped_arguments: false,
                        strict: strict_mode,
                        lexical_this: false,
                        length: 1,
                    });

                    Ok(ObjectEntry::Setter {
                        key: self.lower_prop_name(&setter.key)?,
                        setter: Expression::Identifier(generated_name),
                    })
                }
                Prop::KeyValue(property) => Ok(ObjectEntry::Data {
                    key: self.lower_prop_name(&property.key)?,
                    value: self.lower_expression(&property.value)?,
                }),
                _ => {
                    bail!(
                        "only shorthand, key/value, method, getter, and setter object properties are supported"
                    )
                }
            },
        }
    }

    pub(crate) fn lower_template_expression(
        &mut self,
        template: &swc_ecma_ast::Tpl,
    ) -> Result<Expression> {
        let expressions = template
            .exprs
            .iter()
            .map(|expression| self.lower_expression(expression))
            .collect::<Result<Vec<_>>>()?;
        self.build_template_expression(template, &expressions)
    }

    pub(crate) fn lower_template_expression_with_substitution(
        &mut self,
        template: &swc_ecma_ast::Tpl,
        index: usize,
        substitution: Expression,
    ) -> Result<Expression> {
        let mut expressions = Vec::with_capacity(template.exprs.len());
        for (expression_index, expression) in template.exprs.iter().enumerate() {
            if expression_index == index {
                expressions.push(substitution.clone());
            } else {
                expressions.push(self.lower_expression(expression)?);
            }
        }
        self.build_template_expression(template, &expressions)
    }

    pub(crate) fn build_template_expression(
        &mut self,
        template: &swc_ecma_ast::Tpl,
        expressions: &[Expression],
    ) -> Result<Expression> {
        let mut parts = Vec::new();
        for (index, quasi) in template.quasis.iter().enumerate() {
            parts.push(Expression::String(template_quasi_text(quasi)?));
            if let Some(expression) = expressions.get(index) {
                parts.push(expression.clone());
            }
        }

        let mut expression = parts
            .into_iter()
            .reduce(|left, right| Expression::Binary {
                op: BinaryOp::Add,
                left: Box::new(left),
                right: Box::new(right),
            })
            .unwrap_or(Expression::String(String::new()));
        if !matches!(expression, Expression::String(_)) {
            expression = Expression::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expression::String(String::new())),
                right: Box::new(expression),
            };
        }
        Ok(expression)
    }

    pub(crate) fn lower_prop_name(&mut self, name: &PropName) -> Result<Expression> {
        Ok(match name {
            PropName::Ident(identifier) => Expression::String(identifier.sym.to_string()),
            PropName::Str(string) => {
                Expression::String(string.value.to_string_lossy().into_owned())
            }
            PropName::Num(number) => Expression::Number(number.value),
            PropName::Computed(computed) => self.lower_expression(&computed.expr)?,
            _ => bail!("unsupported object property key"),
        })
    }

    pub(crate) fn lower_member_property(&mut self, property: &MemberProp) -> Result<Expression> {
        Ok(match property {
            MemberProp::Ident(identifier) => Expression::String(identifier.sym.to_string()),
            MemberProp::Computed(computed) => self.lower_expression(&computed.expr)?,
            MemberProp::PrivateName(private_name) => self.lower_private_name(private_name)?,
        })
    }

    pub(crate) fn lower_super_property(&mut self, property: &SuperPropExpr) -> Result<Expression> {
        Ok(match &property.prop {
            SuperProp::Ident(identifier) => Expression::String(identifier.sym.to_string()),
            SuperProp::Computed(computed) => self.lower_expression(&computed.expr)?,
        })
    }

    pub(crate) fn try_lower_top_level_this_member_update(
        &mut self,
        expression: &Expr,
    ) -> Result<Option<String>> {
        if self.module_mode || self.strict_modes.len() != 1 {
            return Ok(None);
        }

        let Expr::Member(member) = expression else {
            return Ok(None);
        };
        if !matches!(member.obj.as_ref(), Expr::This(_)) {
            return Ok(None);
        }

        let Some(name) = static_member_property_name(&member.prop) else {
            return Ok(None);
        };
        Ok(Some(self.resolve_binding_name(&name)))
    }
}
