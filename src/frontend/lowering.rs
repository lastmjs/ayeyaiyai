#[derive(Default)]
struct Lowerer {
    source_text: Option<String>,
    functions: Vec<FunctionDeclaration>,
    next_function_expression_id: usize,
    next_temporary_id: usize,
    binding_scopes: Vec<BindingScope>,
    active_binding_counts: HashMap<String, usize>,
    private_name_scopes: Vec<HashMap<String, String>>,
    constructor_super_stack: Vec<Option<String>>,
    strict_modes: Vec<bool>,
    module_mode: bool,
    current_module_path: Option<PathBuf>,
    module_index_lookup: HashMap<PathBuf, usize>,
}

impl Lowerer {
    fn source_span_snippet(&self, span: Span) -> Option<&str> {
        let source = self.source_text.as_deref()?;
        if span.lo.is_dummy() || span.hi.is_dummy() {
            return None;
        }
        let start = span.lo.0.saturating_sub(1) as usize;
        let end = span.hi.0.saturating_sub(1) as usize;
        source.get(start..end)
    }

    fn pure_array_pattern_elision_count(&self, array: &swc_ecma_ast::ArrayPat) -> usize {
        if !array.elems.is_empty() {
            return 0;
        }
        let Some(snippet) = self.source_span_snippet(array.span) else {
            return 0;
        };
        let Some(inner) = snippet
            .strip_prefix('[')
            .and_then(|text| text.strip_suffix(']'))
        else {
            return 0;
        };
        if inner
            .chars()
            .all(|character| character.is_whitespace() || character == ',')
        {
            inner.chars().filter(|&character| character == ',').count()
        } else {
            0
        }
    }

    fn lower_program(&mut self, program: &SwcProgram) -> Result<Program> {
        let mut statements = Vec::new();
        let strict_mode = match program {
            SwcProgram::Script(script) => script_has_use_strict_directive(&script.body),
            SwcProgram::Module(_) => true,
        };
        self.strict_modes.push(strict_mode);
        self.module_mode = matches!(program, SwcProgram::Module(_));

        match program {
            SwcProgram::Script(script) => {
                let scope_bindings = collect_direct_statement_lexical_bindings(&script.body)?;
                self.push_binding_scope(scope_bindings);
                let lowered = self.lower_top_level_statements(script.body.iter(), &mut statements);
                self.pop_binding_scope();
                lowered?
            }
            SwcProgram::Module(module) => {
                for item in &module.body {
                    match item {
                        ModuleItem::Stmt(statement) => {
                            self.lower_top_level_statement(statement, &mut statements)?
                        }
                        ModuleItem::ModuleDecl(module_declaration) => {
                            self.lower_module_declaration(module_declaration, &mut statements)?
                        }
                    }
                }
            }
        }

        self.strict_modes.pop();
        self.module_mode = false;
        self.current_module_path = None;
        self.module_index_lookup.clear();

        Ok(self.finish_program(statements, strict_mode))
    }

    fn finish_program(&mut self, statements: Vec<Statement>, strict: bool) -> Program {
        self.module_mode = false;
        self.current_module_path = None;
        self.module_index_lookup.clear();

        let mut functions = Vec::new();
        let mut seen = HashSet::new();
        for function in std::mem::take(&mut self.functions).into_iter().rev() {
            if seen.insert(function.name.clone()) {
                functions.push(function);
            }
        }
        functions.reverse();

        Program {
            strict,
            functions,
            statements,
        }
    }

    fn fresh_temporary_name(&mut self, prefix: &str) -> String {
        self.next_temporary_id += 1;
        format!("__ayy_{prefix}_{}", self.next_temporary_id)
    }

    fn fresh_scoped_binding_name(&mut self, name: &str) -> String {
        self.next_temporary_id += 1;
        format!("__ayy_scope${name}${}", self.next_temporary_id)
    }

    fn push_binding_scope(&mut self, names: Vec<String>) {
        let mut scope = BindingScope::default();

        for name in names {
            if scope.names.contains(&name) {
                continue;
            }

            if self.active_binding_counts.contains_key(&name) {
                scope
                    .renames
                    .insert(name.clone(), self.fresh_scoped_binding_name(&name));
            }

            *self.active_binding_counts.entry(name.clone()).or_insert(0) += 1;
            scope.names.push(name);
        }

        self.binding_scopes.push(scope);
    }

    fn pop_binding_scope(&mut self) {
        let Some(scope) = self.binding_scopes.pop() else {
            return;
        };

        for name in scope.names {
            let Some(count) = self.active_binding_counts.get_mut(&name) else {
                continue;
            };
            *count -= 1;
            if *count == 0 {
                self.active_binding_counts.remove(&name);
            }
        }
    }

    fn resolve_binding_name(&self, name: &str) -> String {
        for scope in self.binding_scopes.iter().rev() {
            if let Some(mapped) = scope.renames.get(name) {
                return mapped.clone();
            }
        }

        name.to_string()
    }

    fn lower_dynamic_import_expression(
        &mut self,
        call: &swc_ecma_ast::CallExpr,
    ) -> Result<Expression> {
        ensure!(
            call.args.len() == 1,
            "dynamic import expects exactly one argument"
        );
        let argument = &call.args[0];
        ensure!(
            argument.spread.is_none(),
            "dynamic import does not support spread arguments"
        );

        let Expr::Lit(Lit::Str(specifier)) = &*argument.expr else {
            bail!("unsupported dynamic import specifier");
        };
        let module_index = self
            .current_module_path
            .as_ref()
            .and_then(|module_path| {
                resolve_module_specifier(module_path, &specifier.value.to_string_lossy()).ok()
            })
            .and_then(|resolved| self.module_index_lookup.get(&resolved).copied())
            .map(|module_index| module_index as f64)
            .unwrap_or(-1.0);

        Ok(Expression::Call {
            callee: Box::new(Expression::Identifier("__ayyDynamicImport".to_string())),
            arguments: vec![CallArgument::Expression(Expression::Number(module_index))],
        })
    }

    fn lower_private_name(&self, private_name: &swc_ecma_ast::PrivateName) -> Result<Expression> {
        let name = private_name.name.to_string();
        for scope in self.private_name_scopes.iter().rev() {
            if let Some(mapped) = scope.get(&name) {
                return Ok(Expression::String(mapped.clone()));
            }
        }

        bail!("unsupported private name reference: #{name}")
    }

    fn class_private_name_map(&self, class: &Class, binding_name: &str) -> HashMap<String, String> {
        let mut names = HashMap::new();
        for member in &class.body {
            match member {
                ClassMember::PrivateProp(property) => {
                    names.insert(
                        property.key.name.to_string(),
                        format!("__ayy$private${binding_name}${}", property.key.name),
                    );
                }
                ClassMember::PrivateMethod(method) => {
                    names.insert(
                        method.key.name.to_string(),
                        format!("__ayy$private${binding_name}${}", method.key.name),
                    );
                }
                _ => {}
            }
        }
        names
    }

    fn current_strict_mode(&self) -> bool {
        self.strict_modes.last().copied().unwrap_or(false)
    }

    fn function_strict_mode(&self, function: &Function) -> bool {
        self.current_strict_mode() || function_has_use_strict_directive(function)
    }

    fn arrow_strict_mode(&self, arrow_expression: &ArrowExpr) -> bool {
        self.current_strict_mode()
            || match &*arrow_expression.body {
                BlockStmtOrExpr::BlockStmt(block) => script_has_use_strict_directive(&block.stmts),
                BlockStmtOrExpr::Expr(_) => false,
            }
    }

    fn function_has_mapped_arguments(&self, function: &Function) -> bool {
        !self.function_strict_mode(function) && function_has_simple_parameter_list(function)
    }

    fn lower_top_level_statements<'a>(
        &mut self,
        statements: impl Iterator<Item = &'a Stmt>,
        lowered_statements: &mut Vec<Statement>,
    ) -> Result<()> {
        for statement in statements {
            self.lower_top_level_statement(statement, lowered_statements)?;
        }

        Ok(())
    }

    fn lower_top_level_statement(
        &mut self,
        statement: &Stmt,
        lowered_statements: &mut Vec<Statement>,
    ) -> Result<()> {
        match statement {
            Stmt::Decl(Decl::Fn(function_declaration)) => {
                if self.module_mode {
                    lowered_statements
                        .extend(self.lower_nested_function_declaration(function_declaration)?);
                } else {
                    let lowered = self.lower_function_declaration(function_declaration, true)?;
                    self.functions.push(lowered);
                }
            }
            other => lowered_statements.extend(self.lower_statement(other, false, false)?),
        }

        Ok(())
    }

    fn lower_module_declaration(
        &mut self,
        module_declaration: &ModuleDecl,
        lowered_statements: &mut Vec<Statement>,
    ) -> Result<()> {
        match module_declaration {
            ModuleDecl::ExportDecl(export) => match &export.decl {
                Decl::Fn(function_declaration) => lowered_statements
                    .extend(self.lower_nested_function_declaration(function_declaration)?),
                Decl::Var(variable_declaration) => lowered_statements
                    .extend(self.lower_variable_declaration(variable_declaration)?),
                other => bail!("unsupported export declaration: {other:?}"),
            },
            ModuleDecl::ExportDefaultDecl(export_default) => {
                lowered_statements.extend(self.lower_export_default_declaration(export_default)?)
            }
            ModuleDecl::ExportDefaultExpr(export_default) => {
                lowered_statements.push(Statement::Expression(
                    self.lower_expression_with_name_hint(&export_default.expr, Some("default"))?,
                ));
            }
            ModuleDecl::ExportNamed(export_named) if export_named.src.is_none() => {}
            ModuleDecl::Import(_) | ModuleDecl::ExportNamed(_) | ModuleDecl::ExportAll(_) => {
                bail!("import and export declarations are not supported yet")
            }
            other => bail!("unsupported module declaration: {other:?}"),
        }

        Ok(())
    }

    fn lower_export_default_declaration(
        &mut self,
        export_default: &ExportDefaultDecl,
    ) -> Result<Vec<Statement>> {
        match &export_default.decl {
            DefaultDecl::Fn(function_expression) => {
                if let Some(identifier) = &function_expression.ident {
                    let generated_name =
                        self.lower_named_default_function_expression(function_expression)?;
                    Ok(vec![Statement::Let {
                        name: identifier.sym.to_string(),
                        mutable: true,
                        value: Expression::Identifier(generated_name),
                    }])
                } else {
                    Ok(vec![Statement::Expression(
                        self.lower_function_expression(function_expression, Some("default"))?,
                    )])
                }
            }
            other => bail!("unsupported default export declaration: {other:?}"),
        }
    }

    fn lower_named_default_function_expression(
        &mut self,
        function_expression: &FnExpr,
    ) -> Result<String> {
        self.next_function_expression_id += 1;
        let identifier = function_expression
            .ident
            .as_ref()
            .context("named default export function must have an identifier")?;
        let generated_name = format!(
            "__ayy_fnstmt_{}_{}",
            identifier.sym, self.next_function_expression_id
        );
        let kind = lower_function_kind(
            function_expression.function.is_generator,
            function_expression.function.is_async,
        );
        let extra_bindings = vec![identifier.sym.to_string()];
        let (params, body) =
            self.lower_function_parts(&function_expression.function, &extra_bindings)?;

        self.functions.push(FunctionDeclaration {
            name: generated_name.clone(),
            top_level_binding: None,
            params,
            body,
            register_global: false,
            kind,
            self_binding: None,
            mapped_arguments: self.function_has_mapped_arguments(&function_expression.function),
            strict: self.function_strict_mode(&function_expression.function),
            lexical_this: false,
            length: expected_argument_count(
                function_expression
                    .function
                    .params
                    .iter()
                    .map(|parameter| &parameter.pat),
            ),
        });

        Ok(generated_name)
    }

    fn lower_function_declaration(
        &mut self,
        function_declaration: &FnDecl,
        register_global: bool,
    ) -> Result<FunctionDeclaration> {
        ensure!(
            !function_declaration.declare,
            "declare function is not supported yet"
        );

        let extra_bindings = vec![function_declaration.ident.sym.to_string()];
        let (params, body) =
            self.lower_function_parts(&function_declaration.function, &extra_bindings)?;

        Ok(FunctionDeclaration {
            name: function_declaration.ident.sym.to_string(),
            top_level_binding: None,
            params,
            body,
            register_global,
            kind: lower_function_kind(
                function_declaration.function.is_generator,
                function_declaration.function.is_async,
            ),
            self_binding: Some(function_declaration.ident.sym.to_string()),
            mapped_arguments: self.function_has_mapped_arguments(&function_declaration.function),
            strict: self.function_strict_mode(&function_declaration.function),
            lexical_this: false,
            length: expected_argument_count(
                function_declaration
                    .function
                    .params
                    .iter()
                    .map(|parameter| &parameter.pat),
            ),
        })
    }

    fn lower_function_expression(
        &mut self,
        function_expression: &FnExpr,
        name_hint: Option<&str>,
    ) -> Result<Expression> {
        self.next_function_expression_id += 1;
        let generated_name = match &function_expression.ident {
            Some(identifier) => {
                format!(
                    "__ayy_fnexpr_{}_{}",
                    identifier.sym, self.next_function_expression_id
                )
            }
            None => match name_hint {
                Some(name_hint) => format!(
                    "__ayy_fnexpr_{}__name_{}",
                    self.next_function_expression_id, name_hint
                ),
                None => format!("__ayy_fnexpr_{}", self.next_function_expression_id),
            },
        };
        let kind = lower_function_kind(
            function_expression.function.is_generator,
            function_expression.function.is_async,
        );
        let extra_bindings = function_expression
            .ident
            .as_ref()
            .map(|identifier| vec![identifier.sym.to_string()])
            .unwrap_or_default();
        let (params, body) =
            self.lower_function_parts(&function_expression.function, &extra_bindings)?;
        let self_binding = function_expression
            .ident
            .as_ref()
            .map(|identifier| identifier.sym.to_string());

        self.functions.push(FunctionDeclaration {
            name: generated_name.clone(),
            top_level_binding: None,
            params,
            body,
            register_global: false,
            kind,
            self_binding,
            mapped_arguments: self.function_has_mapped_arguments(&function_expression.function),
            strict: self.function_strict_mode(&function_expression.function),
            lexical_this: false,
            length: expected_argument_count(
                function_expression
                    .function
                    .params
                    .iter()
                    .map(|parameter| &parameter.pat),
            ),
        });

        Ok(Expression::Identifier(generated_name))
    }

    fn lower_arrow_expression(
        &mut self,
        arrow_expression: &ArrowExpr,
        name_hint: Option<&str>,
    ) -> Result<Expression> {
        self.next_function_expression_id += 1;
        let generated_name = match name_hint {
            Some(name_hint) => format!(
                "__ayy_arrow_{}__name_{}",
                self.next_function_expression_id, name_hint
            ),
            None => format!("__ayy_arrow_{}", self.next_function_expression_id),
        };

        let (params, param_setup) = lower_parameter_patterns(self, arrow_expression.params.iter())?;

        let mut body = match &*arrow_expression.body {
            BlockStmtOrExpr::BlockStmt(block) => {
                self.lower_statements(&block.stmts, true, false)?
            }
            BlockStmtOrExpr::Expr(expression) => vec![Statement::Return(
                self.lower_expression_with_name_hint(expression, None)?,
            )],
        };
        body.splice(0..0, param_setup);

        self.functions.push(FunctionDeclaration {
            name: generated_name.clone(),
            top_level_binding: None,
            params,
            body,
            register_global: false,
            kind: if arrow_expression.is_async {
                FunctionKind::Async
            } else {
                FunctionKind::Ordinary
            },
            self_binding: None,
            mapped_arguments: false,
            strict: self.arrow_strict_mode(arrow_expression),
            lexical_this: true,
            length: expected_argument_count(arrow_expression.params.iter()),
        });

        Ok(Expression::Identifier(generated_name))
    }

    fn lower_statements(
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

    fn lower_statement(
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

    fn lower_try_statement(
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

    fn lower_catch_clause(
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

    fn lower_generator_statements(
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

    fn lower_generator_statement(
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

    fn lower_generator_loop_body(
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

    fn lower_generator_branch(
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

    fn lower_for_of_statement(
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

    fn lower_for_in_statement(
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

    fn lower_break_statement(
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

    fn lower_continue_statement(
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

    fn lower_switch_statement(
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

    fn lower_switch_case_statements(
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

    fn lower_switch_case_lexical_declaration(
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

    fn lower_labeled_statement(
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

    fn attach_label_to_lowered(
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

    fn lower_generator_with_statement(
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

    fn lower_generator_with_body(
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

    fn lower_generator_with_expression(
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

    fn lower_for_of_binding(&mut self, head: &ForHead, value: Expression) -> Result<ForOfBinding> {
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

    fn lower_for_of_pattern_binding(
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

    fn emit_require_object_coercible_check(
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

    fn lower_for_of_expression_target(&mut self, expression: &Expr) -> Result<AssignmentTarget> {
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

    fn lower_expression_with_generator_bindings(
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

    fn lower_generator_assignment_expression(
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

    fn lower_generator_assignment_value(
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

    fn lower_generator_effect_expression(
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

    fn lower_generator_effect_yield_value(&mut self, expression: &Expr) -> Result<Expression> {
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

    fn lower_generator_template_value(
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

    fn lower_generator_yield_statement(
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

    fn lower_expression_statement(&mut self, expression: &Expr) -> Result<Vec<Statement>> {
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

    fn lower_assert_throws_statement(
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

    fn lower_assignment_target(&mut self, target: &AssignTarget) -> Result<AssignmentTarget> {
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

    fn lower_block_or_statement(
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

    fn lower_optional_else(
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

    fn lower_expression(&mut self, expression: &Expr) -> Result<Expression> {
        self.lower_expression_with_name_hint(expression, None)
    }

    fn lower_expression_with_name_hint(
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

    fn lower_assignment_expression(
        &mut self,
        target: AssignmentTarget,
        value: Expression,
    ) -> Result<Expression> {
        Ok(target.into_expression(value))
    }

    fn lower_logical_assignment_expression(
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

    fn lower_variable_declaration(
        &mut self,
        variable_declaration: &swc_ecma_ast::VarDecl,
    ) -> Result<Vec<Statement>> {
        let mut lowered = Vec::new();

        for declarator in &variable_declaration.decls {
            if let Pat::Ident(identifier) = &declarator.name {
                let name = self.resolve_binding_name(identifier.id.sym.as_ref());

                if matches!(variable_declaration.kind, VarDeclKind::Var) {
                    let value = match declarator.init.as_deref() {
                        Some(initializer) => self.lower_expression_with_name_hint(
                            initializer,
                            Some(identifier.id.sym.as_ref()),
                        )?,
                        None => Expression::Undefined,
                    };

                    lowered.push(Statement::Var { name, value });
                } else {
                    let value = match declarator.init.as_deref() {
                        Some(initializer) => self.lower_expression_with_name_hint(
                            initializer,
                            Some(identifier.id.sym.as_ref()),
                        )?,
                        None => Expression::Undefined,
                    };

                    lowered.push(Statement::Let {
                        name,
                        mutable: !matches!(variable_declaration.kind, VarDeclKind::Const),
                        value,
                    });
                }

                continue;
            }

            if matches!(variable_declaration.kind, VarDeclKind::Var) {
                let mut names = Vec::new();
                collect_pattern_binding_names(&declarator.name, &mut names)?;
                for name in names {
                    lowered.push(Statement::Var {
                        name,
                        value: Expression::Undefined,
                    });
                }
            }

            let temporary_name = self.fresh_temporary_name("decl");
            let value = match declarator.init.as_deref() {
                Some(initializer) => self.lower_expression_with_name_hint(
                    initializer,
                    pattern_name_hint(&declarator.name),
                )?,
                None => Expression::Undefined,
            };
            lowered.push(Statement::Let {
                name: temporary_name.clone(),
                mutable: true,
                value,
            });
            self.lower_for_of_pattern_binding(
                &declarator.name,
                Expression::Identifier(temporary_name),
                if matches!(variable_declaration.kind, VarDeclKind::Var) {
                    ForOfPatternBindingKind::Assignment
                } else {
                    ForOfPatternBindingKind::Lexical {
                        mutable: !matches!(variable_declaration.kind, VarDeclKind::Const),
                    }
                },
                &mut lowered,
            )?;
        }

        Ok(lowered)
    }

    fn lower_nested_function_declaration(
        &mut self,
        function_declaration: &FnDecl,
    ) -> Result<Vec<Statement>> {
        self.next_function_expression_id += 1;
        let generated_name = format!(
            "__ayy_fnstmt_{}_{}",
            function_declaration.ident.sym, self.next_function_expression_id
        );
        let kind = lower_function_kind(
            function_declaration.function.is_generator,
            function_declaration.function.is_async,
        );
        let extra_bindings = vec![function_declaration.ident.sym.to_string()];
        let (params, body) =
            self.lower_function_parts(&function_declaration.function, &extra_bindings)?;

        self.functions.push(FunctionDeclaration {
            name: generated_name.clone(),
            top_level_binding: None,
            params,
            body,
            register_global: false,
            kind,
            self_binding: None,
            mapped_arguments: self.function_has_mapped_arguments(&function_declaration.function),
            strict: self.function_strict_mode(&function_declaration.function),
            lexical_this: false,
            length: expected_argument_count(
                function_declaration
                    .function
                    .params
                    .iter()
                    .map(|parameter| &parameter.pat),
            ),
        });

        Ok(vec![Statement::Let {
            name: self.resolve_binding_name(function_declaration.ident.sym.as_ref()),
            mutable: true,
            value: Expression::Identifier(generated_name),
        }])
    }

    fn lower_class_declaration(&mut self, class_declaration: &ClassDecl) -> Result<Vec<Statement>> {
        self.lower_class_definition(
            &class_declaration.class,
            self.resolve_binding_name(class_declaration.ident.sym.as_ref()),
        )
    }

    fn lower_class_expression(
        &mut self,
        class_expression: &swc_ecma_ast::ClassExpr,
        name_hint: Option<&str>,
    ) -> Result<Expression> {
        let class_name = class_expression
            .ident
            .as_ref()
            .map(|identifier| identifier.sym.to_string())
            .or_else(|| name_hint.map(str::to_string))
            .unwrap_or_else(|| self.fresh_temporary_name("class_expr"));
        let init_name = self.fresh_temporary_name("class_init");
        let mut init_body =
            self.lower_class_definition(&class_expression.class, class_name.clone())?;
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
            length: 0,
        });

        Ok(Expression::Call {
            callee: Box::new(Expression::Identifier(init_name)),
            arguments: Vec::new(),
        })
    }

    fn lower_class_definition(
        &mut self,
        class: &Class,
        binding_name: String,
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
                value: Expression::Identifier(constructor_name),
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

        for member in &class.body {
            statements.extend(self.lower_class_member(member, &binding_name, &prototype_target)?);
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

    fn lower_class_constructor(
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
            length,
        });

        Ok(generated_name)
    }

    fn lower_class_member(
        &mut self,
        member: &ClassMember,
        class_name: &str,
        prototype_target: &Expression,
    ) -> Result<Vec<Statement>> {
        match member {
            ClassMember::Constructor(_) | ClassMember::Empty(_) | ClassMember::PrivateProp(_) => {
                Ok(Vec::new())
            }
            ClassMember::Method(method) => {
                let property = self.lower_prop_name(&method.key)?;
                let target = if method.is_static {
                    Expression::Identifier(class_name.to_string())
                } else {
                    prototype_target.clone()
                };
                if method.kind == MethodKind::Getter {
                    if let Some(private_alias) =
                        self.lower_private_method_alias_getter(method, &target)?
                    {
                        return Ok(vec![define_property_statement(
                            target,
                            property,
                            data_property_descriptor(private_alias, false, false, true),
                        )]);
                    }
                }
                self.lower_defined_class_method(
                    class_name,
                    prototype_target,
                    method.is_static,
                    method.kind,
                    property,
                    &method.function,
                )
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

    fn lower_private_method_alias_getter(
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

    fn lower_defined_class_method(
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

    fn lower_static_class_method_definition(
        &mut self,
        target: Expression,
        property: Expression,
        descriptor: Expression,
    ) -> Vec<Statement> {
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

    fn lower_class_method_function(&mut self, function: &Function) -> Result<String> {
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
            length: expected_argument_count(function.params.iter().map(|parameter| &parameter.pat)),
        });

        Ok(generated_name)
    }

    fn lower_object_entry(&mut self, property: &PropOrSpread) -> Result<ObjectEntry> {
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

    fn lower_template_expression(&mut self, template: &swc_ecma_ast::Tpl) -> Result<Expression> {
        let expressions = template
            .exprs
            .iter()
            .map(|expression| self.lower_expression(expression))
            .collect::<Result<Vec<_>>>()?;
        self.build_template_expression(template, &expressions)
    }

    fn lower_template_expression_with_substitution(
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

    fn build_template_expression(
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

    fn lower_prop_name(&mut self, name: &PropName) -> Result<Expression> {
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

    fn lower_member_property(&mut self, property: &MemberProp) -> Result<Expression> {
        Ok(match property {
            MemberProp::Ident(identifier) => Expression::String(identifier.sym.to_string()),
            MemberProp::Computed(computed) => self.lower_expression(&computed.expr)?,
            MemberProp::PrivateName(private_name) => self.lower_private_name(private_name)?,
        })
    }

    fn lower_super_property(&mut self, property: &SuperPropExpr) -> Result<Expression> {
        Ok(match &property.prop {
            SuperProp::Ident(identifier) => Expression::String(identifier.sym.to_string()),
            SuperProp::Computed(computed) => self.lower_expression(&computed.expr)?,
        })
    }

    fn try_lower_top_level_this_member_update(
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

    fn lower_function_parts(
        &mut self,
        function: &Function,
        extra_bindings: &[String],
    ) -> Result<(Vec<Parameter>, Vec<Statement>)> {
        let body = function
            .body
            .as_ref()
            .context("functions must have a body")?;
        let strict_mode = self.function_strict_mode(function);
        self.strict_modes.push(strict_mode);
        let mut scope_bindings = collect_parameter_binding_names(
            function.params.iter().map(|parameter| &parameter.pat),
        )?;
        for binding in collect_function_scope_binding_names(&body.stmts)? {
            if !scope_bindings.contains(&binding) {
                scope_bindings.push(binding);
            }
        }
        if !scope_bindings.iter().any(|binding| binding == "arguments") {
            scope_bindings.push("arguments".to_string());
        }
        for binding in extra_bindings {
            if !scope_bindings.contains(binding) {
                scope_bindings.push(binding.clone());
            }
        }

        self.push_binding_scope(scope_bindings);
        let lowered = (|| -> Result<(Vec<Parameter>, Vec<Statement>)> {
            let (params, param_setup) = lower_parameters(self, function)?;
            let mut body = if function.is_generator {
                self.lower_generator_statements(&body.stmts, true)?
            } else {
                self.lower_statements(&body.stmts, true, false)?
            };
            body.splice(0..0, param_setup);
            Ok((params, body))
        })();
        self.pop_binding_scope();
        self.strict_modes.pop();
        lowered.map(|(params, body)| {
            let body = if function.is_async {
                asyncify_statements(body).0
            } else {
                body
            };
            (params, body)
        })
    }
}

fn lower_parameters(
    lowerer: &mut Lowerer,
    function: &Function,
) -> Result<(Vec<Parameter>, Vec<Statement>)> {
    lower_parameter_patterns(
        lowerer,
        function.params.iter().map(|parameter| &parameter.pat),
    )
}

fn collect_parameter_binding_names<'a>(
    parameters: impl IntoIterator<Item = &'a Pat>,
) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for parameter in parameters {
        collect_pattern_binding_names(parameter, &mut names)?;
    }
    Ok(names)
}

fn collect_function_scope_binding_names(statements: &[Stmt]) -> Result<Vec<String>> {
    fn collect_statement(statement: &Stmt, names: &mut Vec<String>) -> Result<()> {
        match statement {
            Stmt::Decl(Decl::Var(variable_declaration))
                if matches!(variable_declaration.kind, VarDeclKind::Var) =>
            {
                for declarator in &variable_declaration.decls {
                    collect_pattern_binding_names(&declarator.name, names)?;
                }
            }
            Stmt::Decl(Decl::Fn(function_declaration)) => {
                let name = function_declaration.ident.sym.to_string();
                if !names.contains(&name) {
                    names.push(name);
                }
            }
            Stmt::Block(block) => {
                for statement in &block.stmts {
                    collect_statement(statement, names)?;
                }
            }
            Stmt::Labeled(labeled_statement) => {
                collect_statement(&labeled_statement.body, names)?;
            }
            Stmt::If(if_statement) => {
                collect_statement(&if_statement.cons, names)?;
                if let Some(alternate) = &if_statement.alt {
                    collect_statement(alternate, names)?;
                }
            }
            Stmt::While(while_statement) => {
                collect_statement(&while_statement.body, names)?;
            }
            Stmt::DoWhile(do_while_statement) => {
                collect_statement(&do_while_statement.body, names)?;
            }
            Stmt::For(for_statement) => {
                if let Some(VarDeclOrExpr::VarDecl(variable_declaration)) = &for_statement.init
                    && matches!(variable_declaration.kind, VarDeclKind::Var)
                {
                    for declarator in &variable_declaration.decls {
                        collect_pattern_binding_names(&declarator.name, names)?;
                    }
                }
                collect_statement(&for_statement.body, names)?;
            }
            Stmt::ForIn(for_in_statement) => {
                if let ForHead::VarDecl(variable_declaration) = &for_in_statement.left
                    && matches!(variable_declaration.kind, VarDeclKind::Var)
                {
                    for declarator in &variable_declaration.decls {
                        collect_pattern_binding_names(&declarator.name, names)?;
                    }
                }
                collect_statement(&for_in_statement.body, names)?;
            }
            Stmt::ForOf(for_of_statement) => {
                if let ForHead::VarDecl(variable_declaration) = &for_of_statement.left
                    && matches!(variable_declaration.kind, VarDeclKind::Var)
                {
                    for declarator in &variable_declaration.decls {
                        collect_pattern_binding_names(&declarator.name, names)?;
                    }
                }
                collect_statement(&for_of_statement.body, names)?;
            }
            Stmt::Switch(switch_statement) => {
                for case in &switch_statement.cases {
                    for statement in &case.cons {
                        collect_statement(statement, names)?;
                    }
                }
            }
            Stmt::Try(try_statement) => {
                for statement in &try_statement.block.stmts {
                    collect_statement(statement, names)?;
                }
                if let Some(handler) = &try_statement.handler {
                    for statement in &handler.body.stmts {
                        collect_statement(statement, names)?;
                    }
                }
                if let Some(finalizer) = &try_statement.finalizer {
                    for statement in &finalizer.stmts {
                        collect_statement(statement, names)?;
                    }
                }
            }
            Stmt::With(with_statement) => {
                collect_statement(&with_statement.body, names)?;
            }
            _ => {}
        }

        Ok(())
    }

    let mut names = Vec::new();
    for statement in statements {
        collect_statement(statement, &mut names)?;
    }
    Ok(names)
}

fn lower_constructor_parameters(
    lowerer: &mut Lowerer,
    constructor: &Constructor,
) -> Result<(Vec<Parameter>, Vec<Statement>, usize)> {
    let mut patterns = Vec::with_capacity(constructor.params.len());
    for parameter in &constructor.params {
        let ParamOrTsParamProp::Param(parameter) = parameter else {
            bail!("parameter properties are not supported yet")
        };
        patterns.push(&parameter.pat);
    }

    let (params, setup) = lower_parameter_patterns(lowerer, patterns.iter().copied())?;
    Ok((
        params,
        setup,
        expected_argument_count(patterns.iter().copied()),
    ))
}

fn lower_parameter_patterns<'a>(
    lowerer: &mut Lowerer,
    parameters: impl IntoIterator<Item = &'a Pat>,
) -> Result<(Vec<Parameter>, Vec<Statement>)> {
    let mut lowered_parameters = Vec::new();
    let mut setup = Vec::new();

    for parameter in parameters {
        let (lowered, mut lowered_setup) = lower_parameter(lowerer, parameter)?;
        lowered_parameters.push(lowered);
        setup.append(&mut lowered_setup);
    }

    Ok((lowered_parameters, setup))
}

fn lower_parameter(lowerer: &mut Lowerer, parameter: &Pat) -> Result<(Parameter, Vec<Statement>)> {
    match parameter {
        Pat::Ident(identifier) => Ok((
            Parameter {
                name: lowerer.resolve_binding_name(identifier.id.sym.as_ref()),
                default: None,
                rest: false,
            },
            Vec::new(),
        )),
        Pat::Assign(assign) => match &*assign.left {
            Pat::Ident(identifier) => Ok((
                Parameter {
                    name: lowerer.resolve_binding_name(identifier.id.sym.as_ref()),
                    default: Some(lowerer.lower_expression(&assign.right)?),
                    rest: false,
                },
                Vec::new(),
            )),
            pattern => {
                let temporary_name = lowerer.fresh_temporary_name("param");
                let mut setup = Vec::new();
                lowerer.lower_for_of_pattern_binding(
                    pattern,
                    Expression::Identifier(temporary_name.clone()),
                    ForOfPatternBindingKind::Lexical { mutable: true },
                    &mut setup,
                )?;
                Ok((
                    Parameter {
                        name: temporary_name,
                        default: Some(lowerer.lower_expression(&assign.right)?),
                        rest: false,
                    },
                    setup,
                ))
            }
        },
        Pat::Rest(rest) => {
            if let Ok(BindingIdent { id, .. }) = binding_ident(&rest.arg) {
                return Ok((
                    Parameter {
                        name: lowerer.resolve_binding_name(id.sym.as_ref()),
                        default: None,
                        rest: true,
                    },
                    Vec::new(),
                ));
            }

            let temporary_name = lowerer.fresh_temporary_name("rest");
            let mut setup = Vec::new();
            lowerer.lower_for_of_pattern_binding(
                &rest.arg,
                Expression::Identifier(temporary_name.clone()),
                ForOfPatternBindingKind::Lexical { mutable: true },
                &mut setup,
            )?;
            Ok((
                Parameter {
                    name: temporary_name,
                    default: None,
                    rest: true,
                },
                setup,
            ))
        }
        pattern => {
            let temporary_name = lowerer.fresh_temporary_name("param");
            let mut setup = Vec::new();
            lowerer.lower_for_of_pattern_binding(
                pattern,
                Expression::Identifier(temporary_name.clone()),
                ForOfPatternBindingKind::Lexical { mutable: true },
                &mut setup,
            )?;
            Ok((
                Parameter {
                    name: temporary_name,
                    default: None,
                    rest: false,
                },
                setup,
            ))
        }
    }
}

fn expected_argument_count<'a>(parameters: impl IntoIterator<Item = &'a Pat>) -> usize {
    let mut count = 0;
    for parameter in parameters {
        match parameter {
            Pat::Rest(_) | Pat::Assign(_) => break,
            _ => count += 1,
        }
    }
    count
}

fn function_has_simple_parameter_list(function: &Function) -> bool {
    function
        .params
        .iter()
        .all(|parameter| matches!(parameter.pat, Pat::Ident(_)))
}

fn collect_for_of_binding_names(pattern: &Pat, names: &mut Vec<String>) -> Result<()> {
    match pattern {
        Pat::Ident(identifier) => {
            let name = identifier.id.sym.to_string();
            if !names.contains(&name) {
                names.push(name);
            }
        }
        Pat::Assign(assign) => collect_for_of_binding_names(&assign.left, names)?,
        Pat::Array(array) => {
            for element in array.elems.iter().flatten() {
                collect_for_of_binding_names(element, names)?;
            }
        }
        _ => bail!("unsupported for-of binding pattern"),
    }

    Ok(())
}

fn collect_switch_bindings(switch_statement: &SwitchStmt) -> Result<Vec<String>> {
    let mut bindings = Vec::new();
    let mut seen = HashSet::new();

    for case in &switch_statement.cases {
        for statement in &case.cons {
            let Stmt::Decl(Decl::Var(variable_declaration)) = statement else {
                continue;
            };
            if matches!(variable_declaration.kind, VarDeclKind::Var) {
                continue;
            }

            for declarator in &variable_declaration.decls {
                let mut names = Vec::new();
                collect_pattern_binding_names(&declarator.name, &mut names)?;
                for name in names {
                    if seen.insert(name.clone()) {
                        bindings.push(name);
                    }
                }
            }
        }
    }

    Ok(bindings)
}

fn collect_direct_statement_lexical_bindings(statements: &[Stmt]) -> Result<Vec<String>> {
    let mut bindings = Vec::new();

    for statement in statements {
        match statement {
            Stmt::Decl(Decl::Var(variable_declaration))
                if !matches!(variable_declaration.kind, VarDeclKind::Var) =>
            {
                for declarator in &variable_declaration.decls {
                    collect_pattern_binding_names(&declarator.name, &mut bindings)?;
                }
            }
            Stmt::Decl(Decl::Fn(function_declaration)) => {
                let name = function_declaration.ident.sym.to_string();
                if !bindings.contains(&name) {
                    bindings.push(name);
                }
            }
            Stmt::Decl(Decl::Class(class_declaration)) => {
                let name = class_declaration.ident.sym.to_string();
                if !bindings.contains(&name) {
                    bindings.push(name);
                }
            }
            _ => {}
        }
    }

    Ok(bindings)
}

fn collect_for_per_iteration_bindings(init: &VarDeclOrExpr) -> Result<Vec<String>> {
    let VarDeclOrExpr::VarDecl(variable_declaration) = init else {
        return Ok(Vec::new());
    };

    if matches!(variable_declaration.kind, VarDeclKind::Var) {
        return Ok(Vec::new());
    }

    Ok(variable_declaration
        .decls
        .iter()
        .map(|declarator| {
            let mut names = Vec::new();
            collect_pattern_binding_names(&declarator.name, &mut names)?;
            Ok(names)
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect())
}

fn pattern_name_hint(pattern: &Pat) -> Option<&str> {
    match pattern {
        Pat::Ident(identifier) => Some(identifier.id.sym.as_ref()),
        _ => None,
    }
}

fn await_resume_expression() -> Expression {
    Expression::Call {
        callee: Box::new(Expression::Identifier("__ayyAwaitResume".to_string())),
        arguments: vec![CallArgument::Expression(Expression::Sent)],
    }
}

fn asyncify_statement(statement: Statement) -> (Vec<Statement>, bool) {
    match statement {
        Statement::Expression(Expression::Await(value)) => (
            vec![
                Statement::Yield { value: *value },
                Statement::Expression(await_resume_expression()),
            ],
            true,
        ),
        Statement::Var {
            name,
            value: Expression::Await(value),
        } => (
            vec![
                Statement::Yield { value: *value },
                Statement::Var {
                    name,
                    value: await_resume_expression(),
                },
            ],
            true,
        ),
        Statement::Let {
            name,
            mutable,
            value: Expression::Await(value),
        } => (
            vec![
                Statement::Yield { value: *value },
                Statement::Let {
                    name,
                    mutable,
                    value: await_resume_expression(),
                },
            ],
            true,
        ),
        Statement::Assign {
            name,
            value: Expression::Await(value),
        } => (
            vec![
                Statement::Yield { value: *value },
                Statement::Assign {
                    name,
                    value: await_resume_expression(),
                },
            ],
            true,
        ),
        Statement::Return(Expression::Await(value)) => (
            vec![
                Statement::Yield { value: *value },
                Statement::Return(await_resume_expression()),
            ],
            true,
        ),
        Statement::If {
            condition: Expression::Await(value),
            then_branch,
            else_branch,
        } => (
            vec![
                Statement::Yield { value: *value },
                Statement::If {
                    condition: await_resume_expression(),
                    then_branch,
                    else_branch,
                },
            ],
            true,
        ),
        other => (vec![other], false),
    }
}

fn asyncify_statements(statements: Vec<Statement>) -> (Vec<Statement>, bool) {
    let mut asyncified = Vec::new();
    let mut changed = false;

    for statement in statements {
        let (mut lowered, statement_changed) = asyncify_statement(statement);
        changed |= statement_changed;
        asyncified.append(&mut lowered);
    }

    (asyncified, changed)
}

fn parse_bigint_literal(value: &str) -> Result<String> {
    Ok(value.to_string())
}

fn template_quasi_text(element: &swc_ecma_ast::TplElement) -> Result<String> {
    if let Some(cooked) = &element.cooked {
        Ok(cooked.to_string_lossy().into_owned())
    } else {
        Ok(element.raw.to_string())
    }
}

fn lower_binary_operator(operator: SwcBinaryOp) -> Result<BinaryOp> {
    Ok(match operator {
        SwcBinaryOp::Add => BinaryOp::Add,
        SwcBinaryOp::Sub => BinaryOp::Subtract,
        SwcBinaryOp::Mul => BinaryOp::Multiply,
        SwcBinaryOp::Div => BinaryOp::Divide,
        SwcBinaryOp::Mod => BinaryOp::Modulo,
        SwcBinaryOp::Exp => BinaryOp::Exponentiate,
        SwcBinaryOp::BitAnd => BinaryOp::BitwiseAnd,
        SwcBinaryOp::BitOr => BinaryOp::BitwiseOr,
        SwcBinaryOp::BitXor => BinaryOp::BitwiseXor,
        SwcBinaryOp::LShift => BinaryOp::LeftShift,
        SwcBinaryOp::RShift => BinaryOp::RightShift,
        SwcBinaryOp::ZeroFillRShift => BinaryOp::UnsignedRightShift,
        SwcBinaryOp::In => BinaryOp::In,
        SwcBinaryOp::InstanceOf => BinaryOp::InstanceOf,
        SwcBinaryOp::EqEq => BinaryOp::LooseEqual,
        SwcBinaryOp::NotEq => BinaryOp::LooseNotEqual,
        SwcBinaryOp::EqEqEq => BinaryOp::Equal,
        SwcBinaryOp::NotEqEq => BinaryOp::NotEqual,
        SwcBinaryOp::Lt => BinaryOp::LessThan,
        SwcBinaryOp::LtEq => BinaryOp::LessThanOrEqual,
        SwcBinaryOp::Gt => BinaryOp::GreaterThan,
        SwcBinaryOp::GtEq => BinaryOp::GreaterThanOrEqual,
        SwcBinaryOp::LogicalAnd => BinaryOp::LogicalAnd,
        SwcBinaryOp::LogicalOr => BinaryOp::LogicalOr,
        SwcBinaryOp::NullishCoalescing => BinaryOp::NullishCoalescing,
    })
}

fn lower_unary_operator(operator: SwcUnaryOp) -> Result<UnaryOp> {
    Ok(match operator {
        SwcUnaryOp::Minus => UnaryOp::Negate,
        SwcUnaryOp::Plus => UnaryOp::Plus,
        SwcUnaryOp::Bang => UnaryOp::Not,
        SwcUnaryOp::Tilde => UnaryOp::BitwiseNot,
        SwcUnaryOp::TypeOf => UnaryOp::TypeOf,
        SwcUnaryOp::Void => UnaryOp::Void,
        SwcUnaryOp::Delete => UnaryOp::Delete,
    })
}

fn lower_update_operator(operator: SwcUpdateOp) -> UpdateOp {
    match operator {
        SwcUpdateOp::PlusPlus => UpdateOp::Increment,
        SwcUpdateOp::MinusMinus => UpdateOp::Decrement,
    }
}

fn static_member_property_name(property: &MemberProp) -> Option<String> {
    match property {
        MemberProp::Ident(identifier) => Some(identifier.sym.to_string()),
        MemberProp::Computed(computed) => match computed.expr.as_ref() {
            Expr::Lit(Lit::Str(string)) => Some(string.value.to_string_lossy().into_owned()),
            _ => None,
        },
        MemberProp::PrivateName(_) => None,
    }
}

fn lower_function_kind(is_generator: bool, is_async: bool) -> FunctionKind {
    if is_generator {
        FunctionKind::Generator
    } else if is_async {
        FunctionKind::Async
    } else {
        FunctionKind::Ordinary
    }
}

fn console_log_arguments(expression: &Expr) -> Option<&[swc_ecma_ast::ExprOrSpread]> {
    let Expr::Call(call) = expression else {
        return None;
    };

    let Callee::Expr(callee) = &call.callee else {
        return None;
    };

    let Expr::Member(member) = &**callee else {
        return None;
    };

    let Expr::Ident(object) = &*member.obj else {
        return None;
    };

    if object.sym != *"console" {
        return None;
    }

    match &member.prop {
        MemberProp::Ident(identifier) if identifier.sym == *"log" => Some(&call.args),
        _ => None,
    }
}

fn assert_throws_call(expression: &Expr) -> Option<&swc_ecma_ast::CallExpr> {
    let Expr::Call(call) = expression else {
        return None;
    };

    let Callee::Expr(callee) = &call.callee else {
        return None;
    };

    let Expr::Ident(identifier) = &**callee else {
        return None;
    };

    (identifier.sym == "__ayyAssertThrows").then_some(call)
}

fn binding_ident(pattern: &Pat) -> Result<&BindingIdent> {
    match pattern {
        Pat::Ident(identifier) => Ok(identifier),
        _ => bail!("only identifier bindings are supported"),
    }
}

enum AssignmentTarget {
    Identifier(String),
    Member {
        object: Expression,
        property: Expression,
    },
    SuperMember {
        property: Expression,
    },
}

impl AssignmentTarget {
    fn as_expression(&self) -> Expression {
        match self {
            AssignmentTarget::Identifier(name) => Expression::Identifier(name.clone()),
            AssignmentTarget::Member { object, property } => Expression::Member {
                object: Box::new(object.clone()),
                property: Box::new(property.clone()),
            },
            AssignmentTarget::SuperMember { property } => Expression::SuperMember {
                property: Box::new(property.clone()),
            },
        }
    }

    fn into_statement(self, value: Expression) -> Statement {
        match self {
            AssignmentTarget::Identifier(name) => Statement::Assign { name, value },
            AssignmentTarget::Member { object, property } => Statement::AssignMember {
                object,
                property,
                value,
            },
            AssignmentTarget::SuperMember { property } => {
                Statement::Expression(Expression::AssignSuperMember {
                    property: Box::new(property),
                    value: Box::new(value),
                })
            }
        }
    }

    fn into_expression(self, value: Expression) -> Expression {
        match self {
            AssignmentTarget::Identifier(name) => Expression::Assign {
                name,
                value: Box::new(value),
            },
            AssignmentTarget::Member { object, property } => Expression::AssignMember {
                object: Box::new(object),
                property: Box::new(property),
                value: Box::new(value),
            },
            AssignmentTarget::SuperMember { property } => Expression::AssignSuperMember {
                property: Box::new(property),
                value: Box::new(value),
            },
        }
    }
}

struct ForOfBinding {
    before_loop: Vec<Statement>,
    per_iteration: Vec<Statement>,
}

#[derive(Clone, Copy)]
enum ForOfPatternBindingKind {
    Assignment,
    Var,
    Lexical { mutable: bool },
}

#[derive(Clone, Copy)]
enum LogicalAssignmentKind {
    And,
    Or,
    Nullish,
}
