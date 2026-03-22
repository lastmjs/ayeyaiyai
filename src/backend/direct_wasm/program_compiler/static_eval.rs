use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn register_static_eval_functions(
        &mut self,
        program: &Program,
    ) -> DirectResult<()> {
        self.register_static_eval_functions_in_statements(&program.statements, None)?;
        for function in &program.functions {
            self.register_static_eval_functions_in_statements(
                &function.body,
                Some(function.name.as_str()),
            )?;
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn register_static_eval_functions_in_statements(
        &mut self,
        statements: &[Statement],
        current_function_name: Option<&str>,
    ) -> DirectResult<()> {
        for statement in statements {
            self.register_static_eval_functions_in_statement(statement, current_function_name)?;
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn register_static_eval_functions_in_statement(
        &mut self,
        statement: &Statement,
        current_function_name: Option<&str>,
    ) -> DirectResult<()> {
        match statement {
            Statement::Block { body } | Statement::Labeled { body, .. } => {
                self.register_static_eval_functions_in_statements(body, current_function_name)?;
            }
            Statement::Var { value, .. }
            | Statement::Let { value, .. }
            | Statement::Assign { value, .. }
            | Statement::Expression(value)
            | Statement::Throw(value)
            | Statement::Return(value)
            | Statement::Yield { value }
            | Statement::YieldDelegate { value } => {
                self.register_static_eval_functions_in_expression(value, current_function_name)?;
            }
            Statement::Print { values } => {
                for value in values {
                    self.register_static_eval_functions_in_expression(
                        value,
                        current_function_name,
                    )?;
                }
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                self.register_static_eval_functions_in_expression(object, current_function_name)?;
                self.register_static_eval_functions_in_expression(property, current_function_name)?;
                self.register_static_eval_functions_in_expression(value, current_function_name)?;
            }
            Statement::With { object, body } => {
                self.register_static_eval_functions_in_expression(object, current_function_name)?;
                self.register_static_eval_functions_in_statements(body, current_function_name)?;
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.register_static_eval_functions_in_expression(
                    condition,
                    current_function_name,
                )?;
                self.register_static_eval_functions_in_statements(
                    then_branch,
                    current_function_name,
                )?;
                self.register_static_eval_functions_in_statements(
                    else_branch,
                    current_function_name,
                )?;
            }
            Statement::Try {
                body,
                catch_setup,
                catch_body,
                ..
            } => {
                self.register_static_eval_functions_in_statements(body, current_function_name)?;
                self.register_static_eval_functions_in_statements(
                    catch_setup,
                    current_function_name,
                )?;
                self.register_static_eval_functions_in_statements(
                    catch_body,
                    current_function_name,
                )?;
            }
            Statement::Switch {
                discriminant,
                cases,
                ..
            } => {
                self.register_static_eval_functions_in_expression(
                    discriminant,
                    current_function_name,
                )?;
                for case in cases {
                    if let Some(test) = &case.test {
                        self.register_static_eval_functions_in_expression(
                            test,
                            current_function_name,
                        )?;
                    }
                    self.register_static_eval_functions_in_statements(
                        &case.body,
                        current_function_name,
                    )?;
                }
            }
            Statement::For {
                init,
                condition,
                update,
                break_hook,
                body,
                ..
            } => {
                self.register_static_eval_functions_in_statements(init, current_function_name)?;
                if let Some(condition) = condition {
                    self.register_static_eval_functions_in_expression(
                        condition,
                        current_function_name,
                    )?;
                }
                if let Some(update) = update {
                    self.register_static_eval_functions_in_expression(
                        update,
                        current_function_name,
                    )?;
                }
                if let Some(break_hook) = break_hook {
                    self.register_static_eval_functions_in_expression(
                        break_hook,
                        current_function_name,
                    )?;
                }
                self.register_static_eval_functions_in_statements(body, current_function_name)?;
            }
            Statement::While {
                condition,
                break_hook,
                body,
                ..
            }
            | Statement::DoWhile {
                condition,
                break_hook,
                body,
                ..
            } => {
                self.register_static_eval_functions_in_expression(
                    condition,
                    current_function_name,
                )?;
                if let Some(break_hook) = break_hook {
                    self.register_static_eval_functions_in_expression(
                        break_hook,
                        current_function_name,
                    )?;
                }
                self.register_static_eval_functions_in_statements(body, current_function_name)?;
            }
            Statement::Break { .. } | Statement::Continue { .. } => {}
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn register_static_eval_functions_in_expression(
        &mut self,
        expression: &Expression,
        current_function_name: Option<&str>,
    ) -> DirectResult<()> {
        match expression {
            Expression::Call { callee, arguments } => {
                if let Some(CallArgument::Expression(Expression::String(source))) =
                    arguments.first()
                {
                    if matches!(callee.as_ref(), Expression::Identifier(name) if name == "eval")
                        && let Some(eval_program) =
                            self.parse_static_eval_program_in_context(source, current_function_name)
                    {
                        self.register_eval_local_function_bindings(
                            current_function_name,
                            &eval_program,
                        );
                        let new_functions = eval_program
                            .functions
                            .iter()
                            .filter(|function| !self.user_function_map.contains_key(&function.name))
                            .cloned()
                            .collect::<Vec<_>>();
                        if !new_functions.is_empty() {
                            self.register_functions(&new_functions)?;
                        }
                        let global_functions = eval_program
                            .functions
                            .iter()
                            .filter(|function| function.register_global)
                            .cloned()
                            .collect::<Vec<_>>();
                        if !global_functions.is_empty() {
                            self.register_functions(&global_functions)?;
                            for function in &global_functions {
                                self.ensure_implicit_global_binding(&function.name);
                                self.global_kinds
                                    .insert(function.name.clone(), StaticValueKind::Function);
                                self.global_value_bindings.insert(
                                    function.name.clone(),
                                    Expression::Identifier(function.name.clone()),
                                );
                                self.global_function_bindings.insert(
                                    function.name.clone(),
                                    LocalFunctionBinding::User(function.name.clone()),
                                );
                            }
                        }
                        self.register_static_eval_functions(&eval_program)?;
                    } else if matches!(
                        callee.as_ref(),
                        Expression::Sequence(expressions)
                            if matches!(expressions.last(), Some(Expression::Identifier(name)) if name == "eval")
                    ) && let Ok(eval_program) = frontend::parse(source)
                    {
                        if !eval_program.strict {
                            for name in collect_eval_var_names(&eval_program) {
                                if self.global_bindings.contains_key(&name) {
                                    continue;
                                }
                                self.ensure_implicit_global_binding(&name);
                            }
                        }
                        let new_functions = eval_program
                            .functions
                            .iter()
                            .filter(|function| !self.user_function_map.contains_key(&function.name))
                            .cloned()
                            .collect::<Vec<_>>();
                        if !new_functions.is_empty() {
                            self.register_functions(&new_functions)?;
                        }
                        self.register_static_eval_functions(&eval_program)?;
                    }
                }
                self.register_static_eval_functions_in_expression(callee, current_function_name)?;
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.register_static_eval_functions_in_expression(
                                expression,
                                current_function_name,
                            )?;
                        }
                    }
                }
            }
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                            self.register_static_eval_functions_in_expression(
                                expression,
                                current_function_name,
                            )?;
                        }
                    }
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        ObjectEntry::Data { key, value } => {
                            self.register_static_eval_functions_in_expression(
                                key,
                                current_function_name,
                            )?;
                            self.register_static_eval_functions_in_expression(
                                value,
                                current_function_name,
                            )?;
                        }
                        ObjectEntry::Getter { key, getter } => {
                            self.register_static_eval_functions_in_expression(
                                key,
                                current_function_name,
                            )?;
                            self.register_static_eval_functions_in_expression(
                                getter,
                                current_function_name,
                            )?;
                        }
                        ObjectEntry::Setter { key, setter } => {
                            self.register_static_eval_functions_in_expression(
                                key,
                                current_function_name,
                            )?;
                            self.register_static_eval_functions_in_expression(
                                setter,
                                current_function_name,
                            )?;
                        }
                        ObjectEntry::Spread(expression) => {
                            self.register_static_eval_functions_in_expression(
                                expression,
                                current_function_name,
                            )?;
                        }
                    }
                }
            }
            Expression::Member { object, property } => {
                self.register_static_eval_functions_in_expression(object, current_function_name)?;
                self.register_static_eval_functions_in_expression(property, current_function_name)?;
            }
            Expression::Assign { value, .. }
            | Expression::Await(value)
            | Expression::EnumerateKeys(value)
            | Expression::GetIterator(value)
            | Expression::IteratorClose(value)
            | Expression::Unary {
                expression: value, ..
            } => {
                self.register_static_eval_functions_in_expression(value, current_function_name)?;
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.register_static_eval_functions_in_expression(object, current_function_name)?;
                self.register_static_eval_functions_in_expression(property, current_function_name)?;
                self.register_static_eval_functions_in_expression(value, current_function_name)?;
            }
            Expression::AssignSuperMember { property, value } => {
                self.register_static_eval_functions_in_expression(property, current_function_name)?;
                self.register_static_eval_functions_in_expression(value, current_function_name)?;
            }
            Expression::Binary { left, right, .. } => {
                self.register_static_eval_functions_in_expression(left, current_function_name)?;
                self.register_static_eval_functions_in_expression(right, current_function_name)?;
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.register_static_eval_functions_in_expression(
                    condition,
                    current_function_name,
                )?;
                self.register_static_eval_functions_in_expression(
                    then_expression,
                    current_function_name,
                )?;
                self.register_static_eval_functions_in_expression(
                    else_expression,
                    current_function_name,
                )?;
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.register_static_eval_functions_in_expression(
                        expression,
                        current_function_name,
                    )?;
                }
            }
            Expression::SuperCall { callee, arguments } | Expression::New { callee, arguments } => {
                self.register_static_eval_functions_in_expression(callee, current_function_name)?;
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.register_static_eval_functions_in_expression(
                                expression,
                                current_function_name,
                            )?;
                        }
                    }
                }
            }
            Expression::SuperMember { property } => {
                self.register_static_eval_functions_in_expression(property, current_function_name)?;
            }
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::Identifier(_)
            | Expression::Update { .. }
            | Expression::NewTarget
            | Expression::This
            | Expression::Sent => {}
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn parse_static_eval_program_in_context(
        &self,
        source: &str,
        current_function_name: Option<&str>,
    ) -> Option<Program> {
        if let Some(current_function_name) = current_function_name {
            if self
                .resolve_home_object_name_for_function_static(current_function_name)
                .is_some()
                && source.contains("super")
                && let Some(program) = self.parse_eval_program_in_method_context_static(source)
            {
                return Some(program);
            }
            if let Some(program) =
                self.parse_eval_program_in_ordinary_function_context_static(source)
            {
                return Some(program);
            }
        }
        frontend::parse(source).ok()
    }

    pub(in crate::backend::direct_wasm) fn resolve_home_object_name_for_function_static(
        &self,
        function_name: &str,
    ) -> Option<String> {
        if let Some(home_object_name) = self
            .user_function_map
            .get(function_name)?
            .home_object_binding
            .as_ref()
        {
            return Some(home_object_name.clone());
        }
        self.global_value_bindings.iter().find_map(|(name, value)| {
            let Expression::Object(entries) = value else {
                return None;
            };
            entries.iter().find_map(|entry| {
                let candidate = match entry {
                    crate::ir::hir::ObjectEntry::Data { value, .. } => value,
                    crate::ir::hir::ObjectEntry::Getter { getter, .. } => getter,
                    crate::ir::hir::ObjectEntry::Setter { setter, .. } => setter,
                    crate::ir::hir::ObjectEntry::Spread(_) => return None,
                };
                matches!(candidate, Expression::Identifier(candidate_name) if candidate_name == function_name)
                    .then_some(name.clone())
            })
        })
    }

    pub(in crate::backend::direct_wasm) fn parse_eval_program_in_ordinary_function_context_static(
        &self,
        source: &str,
    ) -> Option<Program> {
        let wrapper_name = "__ayy_eval_new_target_context__";
        let wrapped_source = format!("function {wrapper_name}() {{\n{source}\n}}");
        let mut wrapped_program = frontend::parse(&wrapped_source).ok()?;
        let wrapper = wrapped_program
            .functions
            .iter()
            .find(|function| function.name == wrapper_name)
            .cloned()?;
        wrapped_program
            .functions
            .retain(|function| function.name != wrapper_name);

        Some(Program {
            strict: wrapper.strict,
            functions: wrapped_program.functions,
            statements: wrapper.body,
        })
    }

    pub(in crate::backend::direct_wasm) fn parse_eval_program_in_method_context_static(
        &self,
        source: &str,
    ) -> Option<Program> {
        let wrapper_property = "__ayy_eval_wrapper__";
        let wrapped_source = format!("({{{wrapper_property}() {{\n{source}\n}}}});");
        let mut wrapped_program = frontend::parse(&wrapped_source).ok()?;
        let wrapper_name = wrapped_program.statements.iter().find_map(|statement| {
            let Statement::Expression(Expression::Object(entries)) = statement else {
                return None;
            };
            entries.iter().find_map(|entry| match entry {
                crate::ir::hir::ObjectEntry::Data { key, value }
                    if matches!(key, Expression::String(name) if name == wrapper_property) =>
                {
                    let Expression::Identifier(name) = value else {
                        return None;
                    };
                    Some(name.clone())
                }
                _ => None,
            })
        })?;
        let wrapper = wrapped_program
            .functions
            .iter()
            .find(|function| function.name == wrapper_name)
            .cloned()?;
        wrapped_program
            .functions
            .retain(|function| function.name != wrapper_name);

        Some(Program {
            strict: wrapper.strict,
            functions: wrapped_program.functions,
            statements: wrapper.body,
        })
    }

    pub(in crate::backend::direct_wasm) fn register_eval_local_function_bindings(
        &mut self,
        current_function_name: Option<&str>,
        program: &Program,
    ) {
        let Some(current_function_name) = current_function_name else {
            return;
        };
        let Some(current_function) = self.user_function_map.get(current_function_name) else {
            return;
        };
        let current_function_strict = current_function.strict;
        let current_function_scope_bindings = current_function.scope_bindings.clone();
        if current_function_strict || program.strict {
            return;
        }

        let local_function_names = program
            .functions
            .iter()
            .filter(|function| is_eval_local_function_candidate(function))
            .map(|function| function.name.clone())
            .collect::<HashSet<_>>();
        if local_function_names.is_empty() {
            return;
        }

        let bindings =
            collect_eval_local_function_declarations(&program.statements, &local_function_names)
                .into_keys()
                .collect::<Vec<_>>();
        if bindings.is_empty() {
            return;
        }

        let target_function_names = std::iter::once(current_function_name.to_string())
            .chain(
                program
                    .functions
                    .iter()
                    .map(|function| function.name.clone()),
            )
            .collect::<Vec<_>>();

        for binding_name in bindings {
            if current_function_scope_bindings.contains(&binding_name) {
                continue;
            }
            let hidden_name = format!(
                "__ayy_eval_local_fn_binding__{}__{}",
                current_function_name, binding_name
            );
            self.ensure_implicit_global_binding(&hidden_name);
            for function_name in &target_function_names {
                self.eval_local_function_bindings
                    .entry(function_name.clone())
                    .or_default()
                    .insert(binding_name.clone(), hidden_name.clone());
            }
        }
    }
}
