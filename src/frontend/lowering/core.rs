use super::*;

impl Lowerer {
    pub(crate) fn with_source_text(source_text: String) -> Self {
        Self {
            source_text: Some(source_text),
            ..Self::default()
        }
    }

    pub(crate) fn source_span_snippet(&self, span: Span) -> Option<&str> {
        let source = self.source_text.as_deref()?;
        if span.lo.is_dummy() || span.hi.is_dummy() {
            return None;
        }
        let start = span.lo.0.saturating_sub(1) as usize;
        let end = span.hi.0.saturating_sub(1) as usize;
        source.get(start..end)
    }

    pub(crate) fn pure_array_pattern_elision_count(&self, array: &swc_ecma_ast::ArrayPat) -> usize {
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

    pub(crate) fn lower_program(&mut self, program: &SwcProgram) -> Result<Program> {
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

    pub(crate) fn finish_program(&mut self, statements: Vec<Statement>, strict: bool) -> Program {
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

    pub(crate) fn fresh_temporary_name(&mut self, prefix: &str) -> String {
        self.next_temporary_id += 1;
        format!("__ayy_{prefix}_{}", self.next_temporary_id)
    }

    pub(crate) fn fresh_scoped_binding_name(&mut self, name: &str) -> String {
        self.next_temporary_id += 1;
        format!("__ayy_scope${name}${}", self.next_temporary_id)
    }

    pub(crate) fn push_binding_scope(&mut self, names: Vec<String>) {
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

    pub(crate) fn pop_binding_scope(&mut self) {
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

    pub(crate) fn resolve_binding_name(&self, name: &str) -> String {
        for scope in self.binding_scopes.iter().rev() {
            if let Some(mapped) = scope.renames.get(name) {
                return mapped.clone();
            }
        }

        name.to_string()
    }

    pub(crate) fn lower_dynamic_import_expression(
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

    pub(crate) fn lower_private_name(
        &self,
        private_name: &swc_ecma_ast::PrivateName,
    ) -> Result<Expression> {
        let name = private_name.name.to_string();
        for scope in self.private_name_scopes.iter().rev() {
            if let Some(mapped) = scope.get(&name) {
                return Ok(Expression::String(mapped.clone()));
            }
        }

        bail!("unsupported private name reference: #{name}")
    }

    pub(crate) fn class_private_name_map(
        &self,
        class: &Class,
        binding_name: &str,
    ) -> HashMap<String, String> {
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

    pub(crate) fn current_strict_mode(&self) -> bool {
        self.strict_modes.last().copied().unwrap_or(false)
    }

    pub(crate) fn function_strict_mode(&self, function: &Function) -> bool {
        self.current_strict_mode() || function_has_use_strict_directive(function)
    }

    pub(crate) fn arrow_strict_mode(&self, arrow_expression: &ArrowExpr) -> bool {
        self.current_strict_mode()
            || match &*arrow_expression.body {
                BlockStmtOrExpr::BlockStmt(block) => script_has_use_strict_directive(&block.stmts),
                BlockStmtOrExpr::Expr(_) => false,
            }
    }

    pub(crate) fn function_has_mapped_arguments(&self, function: &Function) -> bool {
        !self.function_strict_mode(function) && function_has_simple_parameter_list(function)
    }
}
