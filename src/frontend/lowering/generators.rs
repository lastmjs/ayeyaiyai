use super::*;

impl Lowerer {
    pub(crate) fn lower_generator_statements(
        &mut self,
        statements: &[Stmt],
        allow_return: bool,
    ) -> Result<Vec<Statement>> {
        let mut lowered = Vec::new();

        for statement in statements {
            lowered.extend(self.lower_generator_statement(statement, allow_return)?);
        }

        Ok(lowered)
    }

    pub(crate) fn lower_generator_statement(
        &mut self,
        statement: &Stmt,
        allow_return: bool,
    ) -> Result<Vec<Statement>> {
        match statement {
            Stmt::Expr(ExprStmt { expr, .. }) => {
                if let Some(lowered) = self.lower_generator_assignment_expression(expr)? {
                    return Ok(lowered);
                }

                if let Some(lowered) = self.lower_generator_effect_expression(expr)? {
                    return Ok(lowered);
                }

                self.lower_expression_statement(expr)
            }
            Stmt::Block(BlockStmt { stmts, .. })
                if stmts
                    .iter()
                    .all(|statement| matches!(statement, Stmt::Expr(_) | Stmt::Empty(_))) =>
            {
                self.lower_generator_statements(stmts, allow_return)
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
                body: self.lower_generator_loop_body(&for_statement.body, allow_return)?,
            }]),
            Stmt::ForOf(for_of_statement) => {
                self.lower_for_of_statement(for_of_statement, allow_return)
            }
            Stmt::ForIn(for_in_statement) => {
                self.lower_for_in_statement(for_in_statement, allow_return)
            }
            Stmt::If(if_statement) => Ok(vec![Statement::If {
                condition: self.lower_expression(&if_statement.test)?,
                then_branch: self.lower_generator_branch(&if_statement.cons, allow_return)?,
                else_branch: if let Some(alternate) = &if_statement.alt {
                    self.lower_generator_branch(alternate, allow_return)?
                } else {
                    Vec::new()
                },
            }]),
            Stmt::DoWhile(do_while_statement) => Ok(vec![Statement::DoWhile {
                labels: Vec::new(),
                condition: self.lower_expression(&do_while_statement.test)?,
                break_hook: None,
                body: self.lower_generator_loop_body(&do_while_statement.body, allow_return)?,
            }]),
            Stmt::Labeled(labeled_statement) => {
                self.lower_labeled_statement(labeled_statement, allow_return, false)
            }
            Stmt::With(with_statement) => {
                self.lower_generator_with_statement(with_statement, allow_return)
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
            Stmt::Empty(_) => Ok(Vec::new()),
            other => self.lower_statement(other, allow_return, false),
        }
    }

    pub(crate) fn lower_generator_loop_body(
        &mut self,
        statement: &Stmt,
        allow_return: bool,
    ) -> Result<Vec<Statement>> {
        match statement {
            Stmt::Block(BlockStmt { stmts, .. }) => {
                self.lower_generator_statements(stmts, allow_return)
            }
            other => self.lower_generator_statement(other, allow_return),
        }
    }

    pub(crate) fn lower_generator_branch(
        &mut self,
        statement: &Stmt,
        allow_return: bool,
    ) -> Result<Vec<Statement>> {
        match statement {
            Stmt::Block(BlockStmt { stmts, .. }) => {
                self.lower_generator_statements(stmts, allow_return)
            }
            other => self.lower_generator_statement(other, allow_return),
        }
    }

    pub(crate) fn lower_generator_with_statement(
        &mut self,
        with_statement: &WithStmt,
        allow_return: bool,
    ) -> Result<Vec<Statement>> {
        let Expr::Object(object) = &*with_statement.obj else {
            bail!("only object literal `with` is supported in generator functions")
        };

        let mut bindings = HashMap::new();
        for property in &object.props {
            match property {
                PropOrSpread::Prop(property) => match &**property {
                    Prop::KeyValue(property) => {
                        let key = match &property.key {
                            PropName::Ident(identifier) => identifier.sym.to_string(),
                            PropName::Str(string) => string.value.to_string_lossy().into_owned(),
                            _ => bail!("unsupported `with` property key"),
                        };
                        bindings.insert(key, self.lower_expression(&property.value)?);
                    }
                    _ => bail!("unsupported `with` object property"),
                },
                PropOrSpread::Spread(_) => bail!("unsupported `with` object spread"),
            }
        }

        self.lower_generator_with_body(&with_statement.body, allow_return, &bindings)
    }

    pub(crate) fn lower_generator_with_body(
        &mut self,
        statement: &Stmt,
        allow_return: bool,
        bindings: &HashMap<String, Expression>,
    ) -> Result<Vec<Statement>> {
        match statement {
            Stmt::Block(BlockStmt { stmts, .. }) => {
                let mut lowered = Vec::new();
                for statement in stmts {
                    lowered.extend(self.lower_generator_with_body(
                        statement,
                        allow_return,
                        bindings,
                    )?);
                }
                Ok(lowered)
            }
            Stmt::Expr(ExprStmt { expr, .. }) => {
                self.lower_generator_with_expression(expr, bindings)
            }
            Stmt::Empty(_) => Ok(Vec::new()),
            Stmt::Return(return_statement) => {
                ensure!(allow_return, "`return` is only supported inside functions");
                Ok(vec![Statement::Return(
                    match return_statement.arg.as_deref() {
                        Some(expression) => {
                            self.lower_expression_with_generator_bindings(expression, bindings)?
                        }
                        None => Expression::Undefined,
                    },
                )])
            }
            _ => bail!("unsupported statement inside generator `with`"),
        }
    }

    pub(crate) fn lower_generator_with_expression(
        &mut self,
        expression: &Expr,
        bindings: &HashMap<String, Expression>,
    ) -> Result<Vec<Statement>> {
        let Expr::Yield(yield_expression) = expression else {
            bail!("unsupported expression inside generator `with`")
        };

        if yield_expression.delegate {
            let value = yield_expression
                .arg
                .as_deref()
                .context("`yield*` requires an operand")?;
            return Ok(vec![Statement::YieldDelegate {
                value: self.lower_expression_with_generator_bindings(value, bindings)?,
            }]);
        }

        match yield_expression.arg.as_deref() {
            Some(Expr::Yield(inner_yield)) => {
                ensure!(
                    !inner_yield.delegate,
                    "`yield*` as the operand of another `yield` is not supported yet"
                );
                Ok(vec![
                    Statement::Yield {
                        value: match inner_yield.arg.as_deref() {
                            Some(value) => {
                                self.lower_expression_with_generator_bindings(value, bindings)?
                            }
                            None => Expression::Undefined,
                        },
                    },
                    Statement::Yield {
                        value: Expression::Sent,
                    },
                ])
            }
            Some(value) => Ok(vec![Statement::Yield {
                value: self.lower_expression_with_generator_bindings(value, bindings)?,
            }]),
            None => Ok(vec![Statement::Yield {
                value: Expression::Undefined,
            }]),
        }
    }

    pub(crate) fn lower_expression_with_generator_bindings(
        &mut self,
        expression: &Expr,
        bindings: &HashMap<String, Expression>,
    ) -> Result<Expression> {
        match expression {
            Expr::Ident(identifier) => Ok(bindings
                .get(identifier.sym.as_ref())
                .cloned()
                .unwrap_or(Expression::Identifier(identifier.sym.to_string()))),
            _ => self.lower_expression(expression),
        }
    }

    pub(crate) fn lower_generator_assignment_expression(
        &mut self,
        expression: &Expr,
    ) -> Result<Option<Vec<Statement>>> {
        let Expr::Assign(assignment) = expression else {
            return Ok(None);
        };

        if assignment.op != AssignOp::Assign {
            return Ok(None);
        }

        let Some((mut lowered, value)) =
            self.lower_generator_assignment_value(&assignment.right)?
        else {
            return Ok(None);
        };

        let target = self.lower_assignment_target(&assignment.left)?;
        lowered.push(target.into_statement(value));
        Ok(Some(lowered))
    }

    pub(crate) fn lower_generator_assignment_value(
        &mut self,
        expression: &Expr,
    ) -> Result<Option<(Vec<Statement>, Expression)>> {
        match expression {
            Expr::Yield(yield_expression) => Ok(Some((
                self.lower_generator_yield_statement(yield_expression)?,
                Expression::Sent,
            ))),
            Expr::Paren(parenthesized) => {
                self.lower_generator_assignment_value(&parenthesized.expr)
            }
            Expr::Tpl(template) => self.lower_generator_template_value(template),
            _ => Ok(None),
        }
    }

    pub(crate) fn lower_generator_effect_expression(
        &mut self,
        expression: &Expr,
    ) -> Result<Option<Vec<Statement>>> {
        match expression {
            Expr::Yield(yield_expression) => Ok(Some(
                self.lower_generator_yield_statement(yield_expression)?,
            )),
            Expr::Paren(parenthesized) => {
                self.lower_generator_effect_expression(&parenthesized.expr)
            }
            Expr::Seq(sequence) => {
                let mut lowered = Vec::new();
                let mut handled = false;
                for expression in &sequence.exprs {
                    if let Some(mut expression_lowered) =
                        self.lower_generator_effect_expression(expression)?
                    {
                        lowered.append(&mut expression_lowered);
                        handled = true;
                    } else {
                        lowered.extend(self.lower_expression_statement(expression)?);
                    }
                }
                Ok(handled.then_some(lowered))
            }
            Expr::Array(array) => {
                let mut lowered = Vec::new();
                let mut handled = false;
                for element in array.elems.iter().flatten() {
                    if let Some(mut expression_lowered) =
                        self.lower_generator_effect_expression(&element.expr)?
                    {
                        lowered.append(&mut expression_lowered);
                        handled = true;
                    } else {
                        lowered.extend(self.lower_expression_statement(&element.expr)?);
                    }
                }
                Ok(handled.then_some(lowered))
            }
            Expr::Cond(conditional) => {
                let Some((mut lowered, condition)) =
                    self.lower_generator_assignment_value(&conditional.test)?
                else {
                    return Ok(None);
                };
                let then_expression = self.lower_generator_effect_yield_value(&conditional.cons)?;
                let else_expression = self.lower_generator_effect_yield_value(&conditional.alt)?;
                lowered.push(Statement::Yield {
                    value: Expression::Conditional {
                        condition: Box::new(condition),
                        then_expression: Box::new(then_expression),
                        else_expression: Box::new(else_expression),
                    },
                });
                Ok(Some(lowered))
            }
            Expr::Bin(binary) => {
                let left_lowered = self.lower_generator_assignment_value(&binary.left)?;
                let right_lowered = self.lower_generator_assignment_value(&binary.right)?;

                if left_lowered.is_none() && right_lowered.is_none() {
                    return Ok(None);
                }

                let mut lowered = Vec::new();
                let mut left = match left_lowered {
                    Some((mut statements, expression)) => {
                        lowered.append(&mut statements);
                        expression
                    }
                    None => self.lower_expression(&binary.left)?,
                };

                if right_lowered.is_some() {
                    let temporary = self.fresh_temporary_name("generator_bin_left");
                    lowered.push(Statement::Let {
                        name: temporary.clone(),
                        mutable: false,
                        value: left,
                    });
                    left = Expression::Identifier(temporary);
                }

                let right = match right_lowered {
                    Some((mut statements, expression)) => {
                        lowered.append(&mut statements);
                        expression
                    }
                    None => self.lower_expression(&binary.right)?,
                };

                lowered.push(Statement::Expression(Expression::Binary {
                    op: lower_binary_operator(binary.op)?,
                    left: Box::new(left),
                    right: Box::new(right),
                }));
                Ok(Some(lowered))
            }
            _ => Ok(None),
        }
    }

    pub(crate) fn lower_generator_effect_yield_value(
        &mut self,
        expression: &Expr,
    ) -> Result<Expression> {
        let Expr::Yield(yield_expression) = expression else {
            bail!("unsupported generator effect expression")
        };
        ensure!(
            !yield_expression.delegate,
            "`yield*` is not supported in generator effect branches yet"
        );
        match yield_expression.arg.as_deref() {
            Some(value) => self.lower_expression(value),
            None => Ok(Expression::Undefined),
        }
    }

    pub(crate) fn lower_generator_template_value(
        &mut self,
        template: &swc_ecma_ast::Tpl,
    ) -> Result<Option<(Vec<Statement>, Expression)>> {
        let mut yield_index = None;
        let mut yield_expression = None;

        for (index, expression) in template.exprs.iter().enumerate() {
            if let Expr::Yield(candidate) = &**expression {
                ensure!(
                    yield_index.is_none(),
                    "multiple yield expressions in template literals are not supported yet"
                );
                yield_index = Some(index);
                yield_expression = Some(candidate);
            }
        }

        let Some(yield_index) = yield_index else {
            return Ok(None);
        };

        let lowered = self.lower_generator_yield_statement(
            yield_expression.expect("yield expression must exist"),
        )?;
        let expression = self.lower_template_expression_with_substitution(
            template,
            yield_index,
            Expression::Sent,
        )?;
        Ok(Some((lowered, expression)))
    }

    pub(crate) fn lower_generator_yield_statement(
        &mut self,
        yield_expression: &swc_ecma_ast::YieldExpr,
    ) -> Result<Vec<Statement>> {
        if yield_expression.delegate {
            let value = yield_expression
                .arg
                .as_deref()
                .context("`yield*` requires an operand")?;
            return Ok(vec![Statement::YieldDelegate {
                value: self.lower_expression(value)?,
            }]);
        }

        match yield_expression.arg.as_deref() {
            None => Ok(vec![Statement::Yield {
                value: Expression::Undefined,
            }]),
            Some(Expr::Yield(inner_yield)) => {
                ensure!(
                    !inner_yield.delegate,
                    "`yield*` as the operand of another `yield` is not supported yet"
                );
                Ok(vec![
                    Statement::Yield {
                        value: match inner_yield.arg.as_deref() {
                            Some(value) => self.lower_expression(value)?,
                            None => Expression::Undefined,
                        },
                    },
                    Statement::Yield {
                        value: Expression::Sent,
                    },
                ])
            }
            Some(value) => Ok(vec![Statement::Yield {
                value: self.lower_expression(value)?,
            }]),
        }
    }
}
