impl DirectWasmCompiler {
    fn compile(&mut self, program: &Program) -> DirectResult<Vec<u8>> {
        self.next_data_offset = DATA_START_OFFSET;
        self.next_user_type_index = USER_TYPE_BASE_INDEX;
        self.user_type_indices.clear();
        self.user_type_arities.clear();
        self.user_functions.clear();
        self.registered_function_declarations.clear();
        self.user_function_map.clear();
        self.user_function_parameter_bindings.clear();
        self.user_function_parameter_value_bindings.clear();
        self.user_function_parameter_array_bindings.clear();
        self.user_function_parameter_object_bindings.clear();
        self.global_bindings.clear();
        self.global_lexical_bindings.clear();
        self.global_kinds.clear();
        self.global_value_bindings.clear();
        self.global_array_bindings.clear();
        self.global_object_bindings.clear();
        self.global_property_descriptors.clear();
        self.global_object_prototype_bindings.clear();
        self.global_runtime_prototype_bindings.clear();
        self.global_prototype_object_bindings.clear();
        self.global_arguments_bindings.clear();
        self.global_function_bindings.clear();
        self.global_specialized_function_values.clear();
        self.implicit_global_bindings.clear();
        self.global_proxy_bindings.clear();
        self.global_member_function_bindings.clear();
        self.global_member_getter_bindings.clear();
        self.global_member_setter_bindings.clear();
        self.eval_local_function_bindings.clear();
        self.user_function_capture_bindings.clear();
        self.next_test262_realm_id = 0;
        self.test262_realms.clear();
        self.string_data.clear();
        self.interned_strings.clear();

        self.register_functions(&program.functions)?;
        self.register_static_eval_functions(program)?;
        self.register_global_bindings(&program.statements);
        self.register_global_function_bindings(&program.functions);
        (
            self.user_function_parameter_bindings,
            self.user_function_parameter_value_bindings,
            self.user_function_parameter_array_bindings,
            self.user_function_parameter_object_bindings,
        ) = self.collect_user_function_parameter_bindings(program);
        self.register_user_function_capture_bindings(&program.functions);
        self.reserve_function_constructor_implicit_global_bindings(program)?;
        self.reserve_global_runtime_prototype_binding_globals();

        let registered_function_declarations = self.registered_function_declarations.clone();
        let compiled_functions = registered_function_declarations
            .iter()
            .map(|function| self.compile_user_function(function))
            .collect::<DirectResult<Vec<_>>>()?;
        let compiled_start =
            self.compile_start(&program.statements, &program.functions, program.strict)?;

        let (int_min_ptr, int_min_len) = self.intern_string(b"-2147483648".to_vec());

        let mut module = Vec::from(WASM_MAGIC_AND_VERSION);
        push_section(&mut module, 1, encode_type_section(&self.user_type_arities));
        push_section(&mut module, 2, encode_import_section());
        push_section(
            &mut module,
            3,
            encode_function_section(&self.user_functions),
        );
        push_section(&mut module, 5, encode_memory_section());
        push_section(
            &mut module,
            6,
            encode_global_section(
                self.global_bindings.len() as u32
                    + self.implicit_global_bindings.len() as u32 * 2
                    + self.global_runtime_prototype_bindings.len() as u32,
            ),
        );
        push_section(&mut module, 7, encode_export_section());
        push_section(
            &mut module,
            10,
            encode_code_section(compiled_start, compiled_functions, int_min_ptr, int_min_len),
        );
        push_section(&mut module, 11, encode_data_section(&self.string_data));

        Ok(module)
    }

    fn allocate_test262_realm(&mut self) -> u32 {
        let realm_id = self.next_test262_realm_id;
        self.next_test262_realm_id += 1;

        let eval_builtin_name = test262_realm_eval_builtin_name(realm_id);
        let mut global_object_binding = empty_object_value_binding();
        object_binding_set_property(
            &mut global_object_binding,
            Expression::String("eval".to_string()),
            Expression::Identifier(eval_builtin_name),
        );

        self.test262_realms.insert(
            realm_id,
            Test262Realm {
                global_object_binding,
            },
        );
        realm_id
    }

    fn test262_realm_object_binding(&self, realm_id: u32) -> Option<ObjectValueBinding> {
        self.test262_realms.get(&realm_id)?;
        let mut realm_object_binding = empty_object_value_binding();
        object_binding_set_property(
            &mut realm_object_binding,
            Expression::String("global".to_string()),
            Expression::Identifier(test262_realm_global_identifier(realm_id)),
        );
        Some(realm_object_binding)
    }

    fn register_functions(&mut self, functions: &[FunctionDeclaration]) -> DirectResult<()> {
        let function_names = functions
            .iter()
            .map(|function| function.name.clone())
            .collect::<HashSet<_>>();
        for function in functions {
            let arguments_usage = if function.lexical_this {
                ArgumentsUsage::default()
            } else {
                collect_arguments_usage_from_statements(&function.body)
            };
            let extra_argument_indices = arguments_usage
                .indexed_slots
                .into_iter()
                .filter(|index| *index >= function.params.len() as u32)
                .collect::<Vec<_>>();
            let arity = function.params.len() as u32 + 1 + extra_argument_indices.len() as u32;
            let type_index = if let Some(type_index) = self.user_type_indices.get(&arity) {
                *type_index
            } else {
                let type_index = self.next_user_type_index;
                self.next_user_type_index += 1;
                self.user_type_indices.insert(arity, type_index);
                self.user_type_arities.push(arity);
                type_index
            };
            let user_function = UserFunction {
                name: function.name.clone(),
                kind: function.kind,
                params: function
                    .params
                    .iter()
                    .map(|parameter| parameter.name.clone())
                    .collect(),
                scope_bindings: collect_function_constructor_local_bindings(function),
                parameter_defaults: function
                    .params
                    .iter()
                    .map(|parameter| parameter.default.clone())
                    .collect(),
                body_declares_arguments_binding:
                    collect_declared_bindings_from_statements_recursive(&function.body)
                        .contains("arguments"),
                length: function.length as u32,
                extra_argument_indices,
                enumerated_keys_param_index: collect_enumerated_keys_param_index(function),
                returns_arguments_object: function_returns_arguments_object(&function.body),
                returned_arguments_effects: collect_returned_arguments_effects(&function.body),
                returned_member_function_bindings: collect_returned_member_function_bindings(
                    &function.body,
                    &function_names,
                ),
                returned_member_value_bindings: collect_returned_member_value_bindings(
                    &function.body,
                ),
                inline_summary: collect_inline_function_summary(function),
                home_object_binding: None,
                strict: function.strict,
                lexical_this: function.lexical_this,
                function_index: USER_FUNCTION_BASE_INDEX + self.user_functions.len() as u32,
                type_index,
            };
            self.user_functions.push(user_function.clone());
            self.registered_function_declarations.push(function.clone());
            self.user_function_map
                .insert(function.name.clone(), user_function);
            self.register_returned_function_object_bindings(function);
        }

        Ok(())
    }

    fn register_returned_function_object_bindings(&mut self, function: &FunctionDeclaration) {
        let Some(Expression::Identifier(returned_function_name)) =
            collect_returned_identifier_source_expression(&function.body)
        else {
            return;
        };
        if !self.user_function_map.contains_key(&returned_function_name) {
            return;
        }
        let Some(user_function) = self.user_function_map.get(&function.name) else {
            return;
        };
        if user_function.returned_member_value_bindings.is_empty() {
            return;
        }
        let object_binding = self
            .global_object_bindings
            .entry(returned_function_name)
            .or_insert_with(empty_object_value_binding);
        for binding in &user_function.returned_member_value_bindings {
            object_binding_set_property(
                object_binding,
                Expression::String(binding.property.clone()),
                binding.value.clone(),
            );
        }
    }

    fn register_static_eval_functions(&mut self, program: &Program) -> DirectResult<()> {
        self.register_static_eval_functions_in_statements(&program.statements, None)?;
        for function in &program.functions {
            self.register_static_eval_functions_in_statements(
                &function.body,
                Some(function.name.as_str()),
            )?;
        }
        Ok(())
    }

    fn register_static_eval_functions_in_statements(
        &mut self,
        statements: &[Statement],
        current_function_name: Option<&str>,
    ) -> DirectResult<()> {
        for statement in statements {
            self.register_static_eval_functions_in_statement(statement, current_function_name)?;
        }
        Ok(())
    }

    fn register_static_eval_functions_in_statement(
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

    fn register_static_eval_functions_in_expression(
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

    fn parse_static_eval_program_in_context(
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

    fn resolve_home_object_name_for_function_static(&self, function_name: &str) -> Option<String> {
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

    fn parse_eval_program_in_ordinary_function_context_static(
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

    fn parse_eval_program_in_method_context_static(&self, source: &str) -> Option<Program> {
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

    fn register_eval_local_function_bindings(
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

    fn infer_global_arguments_binding(
        &self,
        expression: &Expression,
    ) -> Option<ArgumentsValueBinding> {
        match expression {
            Expression::Identifier(name) => self.global_arguments_bindings.get(name).cloned(),
            Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                let user_function = if let Some(LocalFunctionBinding::User(function_name)) =
                    self.global_function_bindings.get(name)
                {
                    self.user_function_map.get(function_name)
                } else if is_internal_user_function_identifier(name) {
                    self.user_function_map.get(name)
                } else {
                    None
                }?;
                if !user_function.returns_arguments_object {
                    return None;
                }
                Some(ArgumentsValueBinding::for_user_function(
                    user_function,
                    expand_static_call_arguments(arguments, &self.global_array_bindings),
                ))
            }
            _ => None,
        }
    }

    fn infer_global_array_binding(&self, expression: &Expression) -> Option<ArrayValueBinding> {
        match expression {
            Expression::Identifier(name) => self.global_array_bindings.get(name).cloned(),
            Expression::EnumerateKeys(value) => self.infer_enumerated_keys_binding(value),
            Expression::Call { callee, arguments } => {
                if let Some(binding) =
                    self.infer_global_builtin_array_call_binding(callee, arguments)
                {
                    return Some(binding);
                }
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                let user_function = if let Some(LocalFunctionBinding::User(function_name)) =
                    self.global_function_bindings.get(name)
                {
                    self.user_function_map.get(function_name)
                } else if is_internal_user_function_identifier(name) {
                    self.user_function_map.get(name)
                } else {
                    None
                }?;
                let param_index = user_function.enumerated_keys_param_index?;
                let argument = match arguments.get(param_index) {
                    Some(CallArgument::Expression(expression))
                    | Some(CallArgument::Spread(expression)) => expression,
                    None => return Some(ArrayValueBinding { values: Vec::new() }),
                };
                self.infer_enumerated_keys_binding(argument)
            }
            Expression::New { callee, arguments } => {
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                let user_function = if let Some(LocalFunctionBinding::User(function_name)) =
                    self.global_function_bindings.get(name)
                {
                    self.user_function_map.get(function_name)
                } else if is_internal_user_function_identifier(name) {
                    self.user_function_map.get(name)
                } else {
                    None
                }?;
                let param_index = user_function.enumerated_keys_param_index?;
                let argument = match arguments.get(param_index) {
                    Some(CallArgument::Expression(expression))
                    | Some(CallArgument::Spread(expression)) => expression,
                    None => return Some(ArrayValueBinding { values: Vec::new() }),
                };
                self.infer_enumerated_keys_binding(argument)
            }
            Expression::Array(elements) => {
                let mut values = Vec::new();
                for element in elements {
                    match element {
                        crate::ir::hir::ArrayElement::Expression(expression) => {
                            values.push(Some(self.materialize_global_expression(expression)));
                        }
                        crate::ir::hir::ArrayElement::Spread(expression) => {
                            if let Some(binding) = self.infer_global_array_binding(expression) {
                                values.extend(binding.values);
                            } else {
                                values.push(Some(self.materialize_global_expression(expression)));
                            }
                        }
                    }
                }
                Some(ArrayValueBinding { values })
            }
            _ => None,
        }
    }

    fn infer_global_object_binding(&self, expression: &Expression) -> Option<ObjectValueBinding> {
        let mut value_bindings = self.global_value_bindings.clone();
        let mut object_bindings = self.global_object_bindings.clone();
        self.infer_global_object_binding_with_state(
            expression,
            &mut value_bindings,
            &mut object_bindings,
        )
    }

    fn infer_global_object_binding_with_state(
        &self,
        expression: &Expression,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<ObjectValueBinding> {
        match expression {
            Expression::Identifier(name) => object_bindings.get(name).cloned().or_else(|| {
                value_bindings
                    .get(name)
                    .cloned()
                    .filter(
                        |value| !matches!(value, Expression::Identifier(alias) if alias == name),
                    )
                    .and_then(|value| {
                        self.infer_global_object_binding_with_state(
                            &value,
                            value_bindings,
                            object_bindings,
                        )
                    })
            }),
            Expression::Object(entries) => {
                let mut object_binding = empty_object_value_binding();
                for entry in entries {
                    match entry {
                        crate::ir::hir::ObjectEntry::Data { key, value } => {
                            let materialized_key = self
                                .materialize_global_expression_with_state(
                                    key,
                                    &HashMap::new(),
                                    value_bindings,
                                    object_bindings,
                                )
                                .unwrap_or_else(|| self.materialize_global_expression(key));
                            let value = self
                                .materialize_global_expression_with_state(
                                    value,
                                    &HashMap::new(),
                                    value_bindings,
                                    object_bindings,
                                )
                                .unwrap_or_else(|| self.materialize_global_expression(value));
                            object_binding_set_property(
                                &mut object_binding,
                                materialized_key,
                                value,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, .. }
                        | crate::ir::hir::ObjectEntry::Setter { key, .. } => {
                            let materialized_key = self
                                .materialize_global_expression_with_state(
                                    key,
                                    &HashMap::new(),
                                    value_bindings,
                                    object_bindings,
                                )
                                .unwrap_or_else(|| self.materialize_global_expression(key));
                            object_binding_set_property(
                                &mut object_binding,
                                materialized_key,
                                Expression::Undefined,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Spread(expression) => {
                            let spread_expression = self
                                .materialize_global_expression_with_state(
                                    expression,
                                    &HashMap::new(),
                                    value_bindings,
                                    object_bindings,
                                )
                                .unwrap_or_else(|| self.materialize_global_expression(expression));
                            if matches!(spread_expression, Expression::Null | Expression::Undefined)
                                || matches!(
                                    &spread_expression,
                                    Expression::Identifier(name)
                                        if name == "undefined"
                                            && !self.global_bindings.contains_key(name)
                                            && !self.global_lexical_bindings.contains(name)
                                )
                            {
                                continue;
                            }
                            let spread_binding = self
                                .infer_global_copy_data_properties_binding_with_state(
                                    &spread_expression,
                                    value_bindings,
                                    object_bindings,
                                )?;
                            merge_enumerable_object_binding(&mut object_binding, &spread_binding);
                        }
                    }
                }
                Some(object_binding)
            }
            _ => None,
        }
    }

    fn infer_global_copy_data_properties_binding(
        &self,
        expression: &Expression,
    ) -> Option<ObjectValueBinding> {
        let mut value_bindings = self.global_value_bindings.clone();
        let mut object_bindings = self.global_object_bindings.clone();
        self.infer_global_copy_data_properties_binding_with_state(
            expression,
            &mut value_bindings,
            &mut object_bindings,
        )
    }

    fn infer_global_copy_data_properties_binding_with_state(
        &self,
        expression: &Expression,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<ObjectValueBinding> {
        let source_binding = self.infer_global_object_binding_with_state(
            expression,
            value_bindings,
            object_bindings,
        )?;
        let mut copied_binding = empty_object_value_binding();
        for (name, _) in &source_binding.string_properties {
            if source_binding
                .non_enumerable_string_properties
                .iter()
                .any(|hidden_name| hidden_name == name)
            {
                continue;
            }
            let property = Expression::String(name.clone());
            let copied_value = self
                .infer_global_member_getter_return_value_with_state(
                    expression,
                    &property,
                    value_bindings,
                    object_bindings,
                )
                .or_else(|| {
                    self.infer_global_object_binding_with_state(
                        expression,
                        value_bindings,
                        object_bindings,
                    )
                    .and_then(|binding| object_binding_lookup_value(&binding, &property).cloned())
                })
                .unwrap_or(Expression::Undefined);
            object_binding_set_property(&mut copied_binding, property, copied_value);
        }
        for (property, _) in &source_binding.symbol_properties {
            let copied_value = self
                .infer_global_member_getter_return_value_with_state(
                    expression,
                    property,
                    value_bindings,
                    object_bindings,
                )
                .or_else(|| {
                    self.infer_global_object_binding_with_state(
                        expression,
                        value_bindings,
                        object_bindings,
                    )
                    .and_then(|binding| object_binding_lookup_value(&binding, property).cloned())
                })
                .unwrap_or(Expression::Undefined);
            object_binding_set_property(&mut copied_binding, property.clone(), copied_value);
        }
        Some(copied_binding)
    }

    fn infer_global_member_getter_return_value(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<Expression> {
        let mut value_bindings = self.global_value_bindings.clone();
        let mut object_bindings = self.global_object_bindings.clone();
        self.infer_global_member_getter_return_value_with_state(
            object,
            property,
            &mut value_bindings,
            &mut object_bindings,
        )
    }

    fn infer_global_member_getter_return_value_with_state(
        &self,
        object: &Expression,
        property: &Expression,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<Expression> {
        let getter_binding = self.infer_global_member_getter_binding(object, property)?;
        self.execute_global_function_binding_with_state(
            &getter_binding,
            &[],
            value_bindings,
            object_bindings,
        )
    }

    fn infer_global_member_getter_binding(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<LocalFunctionBinding> {
        let target = match object {
            Expression::Identifier(name) => MemberFunctionBindingTarget::Identifier(name.clone()),
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "prototype") =>
            {
                let Expression::Identifier(name) = object.as_ref() else {
                    return None;
                };
                MemberFunctionBindingTarget::Prototype(name.clone())
            }
            Expression::New { callee, .. } => {
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                MemberFunctionBindingTarget::Prototype(name.clone())
            }
            _ => return None,
        };
        let property = if let Some(property_name) = static_property_name_from_expression(property) {
            MemberFunctionBindingProperty::String(property_name)
        } else {
            return None;
        };
        let key = MemberFunctionBindingKey { target, property };
        self.global_member_getter_bindings.get(&key).cloned()
    }

    fn infer_global_function_binding_static_return_expression(
        &self,
        binding: &LocalFunctionBinding,
        arguments: &[CallArgument],
    ) -> Option<Expression> {
        let LocalFunctionBinding::User(function_name) = binding else {
            return None;
        };
        let user_function = self.user_function_map.get(function_name)?;
        if let Some(summary) = user_function.inline_summary.as_ref()
            && summary.effects.is_empty()
            && let Some(return_value) = summary.return_value.as_ref()
        {
            return Some(self.materialize_global_expression(
                &self.substitute_global_user_function_argument_bindings(
                    return_value,
                    user_function,
                    arguments,
                ),
            ));
        }
        let function = self
            .registered_function_declarations
            .iter()
            .find(|function| function.name == *function_name)?;
        let [Statement::Return(expression)] = function.body.as_slice() else {
            return None;
        };
        Some(self.materialize_global_expression(
            &self.substitute_global_user_function_argument_bindings(
                expression,
                user_function,
                arguments,
            ),
        ))
    }

    fn infer_global_member_function_binding_property(
        &self,
        property: &Expression,
    ) -> Option<MemberFunctionBindingProperty> {
        let resolved_property = self.materialize_global_expression(property);
        static_property_name_from_expression(&resolved_property)
            .map(MemberFunctionBindingProperty::String)
    }

    fn execute_global_function_binding_with_state(
        &self,
        binding: &LocalFunctionBinding,
        arguments: &[CallArgument],
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<Expression> {
        let LocalFunctionBinding::User(function_name) = binding else {
            return None;
        };
        let user_function = self.user_function_map.get(function_name)?;
        if let Some(summary) = user_function.inline_summary.as_ref()
            && summary.effects.is_empty()
            && let Some(return_value) = summary.return_value.as_ref()
        {
            let substituted = self.substitute_global_user_function_argument_bindings(
                return_value,
                user_function,
                arguments,
            );
            if let Some(materialized) = self.materialize_global_expression_with_state(
                &substituted,
                &HashMap::new(),
                value_bindings,
                object_bindings,
            ) {
                return Some(materialized);
            }
        }

        let function = self
            .registered_function_declarations
            .iter()
            .find(|function| function.name == *function_name)?;
        let mut local_bindings = HashMap::new();
        for statement in &function.body {
            match statement {
                Statement::Var { name, value } | Statement::Let { name, value, .. } => {
                    let value = self.evaluate_global_expression_with_state(
                        value,
                        &mut local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    local_bindings.insert(name.clone(), value);
                }
                Statement::Assign { name, value } => {
                    let value = self.evaluate_global_expression_with_state(
                        value,
                        &mut local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    self.assign_global_expression_with_state(
                        name,
                        value,
                        &mut local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                }
                Statement::AssignMember {
                    object,
                    property,
                    value,
                } => {
                    let property = self.evaluate_global_expression_with_state(
                        property,
                        &mut local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    let value = self.evaluate_global_expression_with_state(
                        value,
                        &mut local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    self.assign_global_member_expression_with_state(
                        object,
                        property,
                        value,
                        &mut local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                }
                Statement::Expression(expression) => {
                    self.evaluate_global_expression_with_state(
                        expression,
                        &mut local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                }
                Statement::Return(expression) => {
                    return self.evaluate_global_expression_with_state(
                        expression,
                        &mut local_bindings,
                        value_bindings,
                        object_bindings,
                    );
                }
                Statement::Block { body } if body.is_empty() => {}
                _ => return None,
            }
        }

        Some(Expression::Undefined)
    }

    fn materialize_global_expression_with_state(
        &self,
        expression: &Expression,
        local_bindings: &HashMap<String, Expression>,
        value_bindings: &HashMap<String, Expression>,
        object_bindings: &HashMap<String, ObjectValueBinding>,
    ) -> Option<Expression> {
        match expression {
            Expression::Identifier(name) => {
                if self.global_kinds.get(name) == Some(&StaticValueKind::Symbol) {
                    return Some(Expression::Identifier(name.clone()));
                }
                if value_bindings.get(name).is_some_and(|value| {
                    matches!(
                        value,
                        Expression::Call { callee, .. }
                            if matches!(callee.as_ref(), Expression::Identifier(symbol_name)
                                if symbol_name == "Symbol"
                                    && !self.global_bindings.contains_key(symbol_name)
                                    && !self.global_lexical_bindings.contains(symbol_name))
                    )
                }) {
                    return Some(Expression::Identifier(name.clone()));
                }
                if let Some(value) = local_bindings.get(name) {
                    return self.materialize_global_expression_with_state(
                        value,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    );
                }
                if let Some(value) = value_bindings.get(name) {
                    if object_bindings.contains_key(name)
                        && matches!(value, Expression::Object(_) | Expression::Identifier(_))
                    {
                        return Some(Expression::Identifier(name.clone()));
                    }
                    if !matches!(value, Expression::Identifier(alias) if alias == name) {
                        return self.materialize_global_expression_with_state(
                            value,
                            local_bindings,
                            value_bindings,
                            object_bindings,
                        );
                    }
                }
                Some(expression.clone())
            }
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::This
            | Expression::NewTarget
            | Expression::Sent => Some(expression.clone()),
            Expression::Member { object, property } => {
                let object_binding = self.resolve_stateful_object_binding_from_expression(
                    object,
                    local_bindings,
                    value_bindings,
                    object_bindings,
                )?;
                let property = self.materialize_global_expression_with_state(
                    property,
                    local_bindings,
                    value_bindings,
                    object_bindings,
                )?;
                if let Some(value) = object_binding_lookup_value(&object_binding, &property) {
                    return self.materialize_global_expression_with_state(
                        value,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    );
                }
                if static_property_name_from_expression(&property).is_some()
                    || object_binding_has_property(&object_binding, &property)
                {
                    return Some(Expression::Undefined);
                }
                None
            }
            Expression::Object(entries) => Some(Expression::Object(
                entries
                    .iter()
                    .map(|entry| match entry {
                        ObjectEntry::Data { key, value } => Some(ObjectEntry::Data {
                            key: self.materialize_global_expression_with_state(
                                key,
                                local_bindings,
                                value_bindings,
                                object_bindings,
                            )?,
                            value: self.materialize_global_expression_with_state(
                                value,
                                local_bindings,
                                value_bindings,
                                object_bindings,
                            )?,
                        }),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()?,
            )),
            Expression::Array(elements) => Some(Expression::Array(
                elements
                    .iter()
                    .map(|element| match element {
                        ArrayElement::Expression(expression) => Some(ArrayElement::Expression(
                            self.materialize_global_expression_with_state(
                                expression,
                                local_bindings,
                                value_bindings,
                                object_bindings,
                            )?,
                        )),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()?,
            )),
            _ => None,
        }
    }

    fn evaluate_global_expression_with_state(
        &self,
        expression: &Expression,
        local_bindings: &mut HashMap<String, Expression>,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<Expression> {
        match expression {
            Expression::Assign { name, value } => {
                let value = self.evaluate_global_expression_with_state(
                    value,
                    local_bindings,
                    value_bindings,
                    object_bindings,
                )?;
                self.assign_global_expression_with_state(
                    name,
                    value,
                    local_bindings,
                    value_bindings,
                    object_bindings,
                )
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                let property = self.evaluate_global_expression_with_state(
                    property,
                    local_bindings,
                    value_bindings,
                    object_bindings,
                )?;
                let value = self.evaluate_global_expression_with_state(
                    value,
                    local_bindings,
                    value_bindings,
                    object_bindings,
                )?;
                self.assign_global_member_expression_with_state(
                    object,
                    property,
                    value,
                    local_bindings,
                    value_bindings,
                    object_bindings,
                )
            }
            Expression::Unary {
                op: UnaryOp::Delete,
                expression,
            } => match expression.as_ref() {
                Expression::Member { object, property } => {
                    let property = self.evaluate_global_expression_with_state(
                        property,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    let target_name = self.resolve_stateful_object_binding_name(
                        object,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    let binding = object_bindings.get_mut(&target_name)?;
                    object_binding_remove_property(binding, &property);
                    Some(Expression::Bool(true))
                }
                _ => Some(Expression::Bool(true)),
            },
            Expression::Update { name, op, prefix } => {
                let current = local_bindings
                    .get(name)
                    .cloned()
                    .or_else(|| value_bindings.get(name).cloned())
                    .unwrap_or(Expression::Undefined);
                let current_number = match current {
                    Expression::Number(value) => value,
                    Expression::Bool(true) => 1.0,
                    Expression::Bool(false) | Expression::Null => 0.0,
                    Expression::Undefined => f64::NAN,
                    _ => return None,
                };
                let next_number = match op {
                    UpdateOp::Increment => current_number + 1.0,
                    UpdateOp::Decrement => current_number - 1.0,
                };
                let next = Expression::Number(next_number);
                self.assign_global_expression_with_state(
                    name,
                    next.clone(),
                    local_bindings,
                    value_bindings,
                    object_bindings,
                )?;
                Some(if *prefix {
                    next
                } else {
                    Expression::Number(current_number)
                })
            }
            Expression::Sequence(expressions) => {
                let mut last = Expression::Undefined;
                for expression in expressions {
                    last = self.evaluate_global_expression_with_state(
                        expression,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                }
                Some(last)
            }
            _ => self.materialize_global_expression_with_state(
                expression,
                local_bindings,
                value_bindings,
                object_bindings,
            ),
        }
    }

    fn assign_global_expression_with_state(
        &self,
        name: &str,
        value: Expression,
        local_bindings: &mut HashMap<String, Expression>,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<Expression> {
        if local_bindings.contains_key(name) {
            local_bindings.insert(name.to_string(), value.clone());
            return Some(value);
        }

        value_bindings.insert(name.to_string(), value.clone());
        if let Some(object_binding) =
            self.infer_global_object_binding_with_state(&value, value_bindings, object_bindings)
        {
            object_bindings.insert(name.to_string(), object_binding);
        } else {
            object_bindings.remove(name);
        }
        Some(value)
    }

    fn assign_global_member_expression_with_state(
        &self,
        object: &Expression,
        property: Expression,
        value: Expression,
        local_bindings: &mut HashMap<String, Expression>,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<Expression> {
        let target_name = self.resolve_stateful_object_binding_name(
            object,
            local_bindings,
            value_bindings,
            object_bindings,
        )?;
        let binding = object_bindings.get_mut(&target_name)?;
        object_binding_set_property(binding, property, value.clone());
        Some(value)
    }

    fn resolve_stateful_object_binding_name(
        &self,
        expression: &Expression,
        local_bindings: &HashMap<String, Expression>,
        value_bindings: &HashMap<String, Expression>,
        object_bindings: &HashMap<String, ObjectValueBinding>,
    ) -> Option<String> {
        match expression {
            Expression::Identifier(name) if object_bindings.contains_key(name) => {
                Some(name.clone())
            }
            Expression::Identifier(name) => local_bindings
                .get(name)
                .or_else(|| value_bindings.get(name))
                .filter(|value| !matches!(value, Expression::Identifier(alias) if alias == name))
                .and_then(|value| {
                    self.resolve_stateful_object_binding_name(
                        value,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )
                }),
            _ => None,
        }
    }

    fn resolve_stateful_object_binding_from_expression(
        &self,
        expression: &Expression,
        local_bindings: &HashMap<String, Expression>,
        value_bindings: &HashMap<String, Expression>,
        object_bindings: &HashMap<String, ObjectValueBinding>,
    ) -> Option<ObjectValueBinding> {
        match expression {
            Expression::Identifier(name) => object_bindings.get(name).cloned().or_else(|| {
                local_bindings
                    .get(name)
                    .or_else(|| value_bindings.get(name))
                    .filter(
                        |value| !matches!(value, Expression::Identifier(alias) if alias == name),
                    )
                    .and_then(|value| {
                        self.resolve_stateful_object_binding_from_expression(
                            value,
                            local_bindings,
                            value_bindings,
                            object_bindings,
                        )
                    })
            }),
            _ => self.infer_global_object_binding_with_state(
                expression,
                &mut value_bindings.clone(),
                &mut object_bindings.clone(),
            ),
        }
    }

    fn infer_enumerated_keys_binding(&self, expression: &Expression) -> Option<ArrayValueBinding> {
        if let Some(array_binding) = self.infer_global_array_binding(expression) {
            return Some(enumerated_keys_from_array_binding(&array_binding));
        }
        if let Some(object_binding) = self.infer_global_object_binding(expression) {
            return Some(enumerated_keys_from_object_binding(&object_binding));
        }
        None
    }

    fn infer_own_property_names_binding(
        &self,
        expression: &Expression,
    ) -> Option<ArrayValueBinding> {
        if let Some(array_binding) = self.infer_global_array_binding(expression) {
            return Some(own_property_names_from_array_binding(&array_binding));
        }
        let object_binding = self.infer_global_object_binding(expression);
        let has_prototype_binding = matches!(
            expression,
            Expression::Identifier(name) if self.global_prototype_object_bindings.contains_key(name)
        );
        if self.infer_global_function_binding(expression).is_some() || has_prototype_binding {
            return Some(own_property_names_from_function_binding(
                object_binding.as_ref(),
            ));
        }
        if let Some(object_binding) = object_binding {
            return Some(own_property_names_from_object_binding(&object_binding));
        }
        None
    }

    fn infer_own_property_symbols_binding(
        &self,
        expression: &Expression,
    ) -> Option<ArrayValueBinding> {
        let object_binding = self.infer_global_object_binding(expression)?;
        Some(own_property_symbols_from_object_binding(&object_binding))
    }

    fn infer_global_builtin_array_call_binding(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Option<ArrayValueBinding> {
        let Expression::Member { object, property } = callee else {
            return None;
        };
        if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Object") {
            return None;
        }
        let [CallArgument::Expression(target), ..] = arguments else {
            return None;
        };
        match property.as_ref() {
            Expression::String(name) if name == "keys" => {
                self.infer_enumerated_keys_binding(target)
            }
            Expression::String(name) if name == "getOwnPropertyNames" => {
                self.infer_own_property_names_binding(target)
            }
            Expression::String(name) if name == "getOwnPropertySymbols" => {
                self.infer_own_property_symbols_binding(target)
            }
            _ => None,
        }
    }

    fn materialize_global_expression(&self, expression: &Expression) -> Expression {
        match expression {
            Expression::Member { object, property } => {
                if let Some(array_binding) = self.infer_global_array_binding(object) {
                    if let Some(index) = argument_index_from_expression(property) {
                        if let Some(Some(value)) = array_binding.values.get(index as usize) {
                            return self.materialize_global_expression(value);
                        }
                        return Expression::Undefined;
                    }
                }
                if let Some(object_binding) = self.infer_global_object_binding(object) {
                    let materialized_property = self.materialize_global_expression(property);
                    if let Some(value) =
                        object_binding_lookup_value(&object_binding, &materialized_property)
                    {
                        return self.materialize_global_expression(value);
                    }
                    if static_property_name_from_expression(&materialized_property).is_some()
                        || object_binding_has_property(&object_binding, &materialized_property)
                    {
                        return Expression::Undefined;
                    }
                }
                if let Expression::String(text) = object.as_ref() {
                    if let Some(index) = argument_index_from_expression(property) {
                        return text
                            .chars()
                            .nth(index as usize)
                            .map(|character| Expression::String(character.to_string()))
                            .unwrap_or(Expression::Undefined);
                    }
                }
                Expression::Member {
                    object: Box::new(self.materialize_global_expression(object)),
                    property: Box::new(self.materialize_global_expression(property)),
                }
            }
            Expression::Unary { op, expression } => Expression::Unary {
                op: *op,
                expression: Box::new(self.materialize_global_expression(expression)),
            },
            Expression::Binary { op, left, right } => Expression::Binary {
                op: *op,
                left: Box::new(self.materialize_global_expression(left)),
                right: Box::new(self.materialize_global_expression(right)),
            },
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => Expression::Conditional {
                condition: Box::new(self.materialize_global_expression(condition)),
                then_expression: Box::new(self.materialize_global_expression(then_expression)),
                else_expression: Box::new(self.materialize_global_expression(else_expression)),
            },
            Expression::Sequence(expressions) => Expression::Sequence(
                expressions
                    .iter()
                    .map(|expression| self.materialize_global_expression(expression))
                    .collect(),
            ),
            Expression::Array(elements) => Expression::Array(
                elements
                    .iter()
                    .map(|element| match element {
                        crate::ir::hir::ArrayElement::Expression(expression) => {
                            crate::ir::hir::ArrayElement::Expression(
                                self.materialize_global_expression(expression),
                            )
                        }
                        crate::ir::hir::ArrayElement::Spread(expression) => {
                            crate::ir::hir::ArrayElement::Spread(
                                self.materialize_global_expression(expression),
                            )
                        }
                    })
                    .collect(),
            ),
            Expression::Object(entries) => Expression::Object(
                entries
                    .iter()
                    .map(|entry| match entry {
                        crate::ir::hir::ObjectEntry::Data { key, value } => {
                            crate::ir::hir::ObjectEntry::Data {
                                key: self.materialize_global_expression(key),
                                value: self.materialize_global_expression(value),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                            crate::ir::hir::ObjectEntry::Getter {
                                key: self.materialize_global_expression(key),
                                getter: self.materialize_global_expression(getter),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                            crate::ir::hir::ObjectEntry::Setter {
                                key: self.materialize_global_expression(key),
                                setter: self.materialize_global_expression(setter),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Spread(expression) => {
                            crate::ir::hir::ObjectEntry::Spread(
                                self.materialize_global_expression(expression),
                            )
                        }
                    })
                    .collect(),
            ),
            Expression::Assign { name, value } => Expression::Assign {
                name: name.clone(),
                value: Box::new(self.materialize_global_expression(value)),
            },
            Expression::AssignMember {
                object,
                property,
                value,
            } => Expression::AssignMember {
                object: Box::new(self.materialize_global_expression(object)),
                property: Box::new(self.materialize_global_expression(property)),
                value: Box::new(self.materialize_global_expression(value)),
            },
            Expression::AssignSuperMember { property, value } => Expression::AssignSuperMember {
                property: Box::new(self.materialize_global_expression(property)),
                value: Box::new(self.materialize_global_expression(value)),
            },
            Expression::Await(value) => {
                Expression::Await(Box::new(self.materialize_global_expression(value)))
            }
            Expression::EnumerateKeys(value) => {
                Expression::EnumerateKeys(Box::new(self.materialize_global_expression(value)))
            }
            Expression::GetIterator(value) => {
                Expression::GetIterator(Box::new(self.materialize_global_expression(value)))
            }
            Expression::IteratorClose(value) => {
                Expression::IteratorClose(Box::new(self.materialize_global_expression(value)))
            }
            Expression::Call { callee, arguments } => {
                if let Some(value) = self.infer_static_call_result_expression(callee, arguments) {
                    return self.materialize_global_expression(&value);
                }
                Expression::Call {
                    callee: Box::new(self.materialize_global_expression(callee)),
                    arguments: arguments
                        .iter()
                        .map(|argument| match argument {
                            CallArgument::Expression(expression) => CallArgument::Expression(
                                self.materialize_global_expression(expression),
                            ),
                            CallArgument::Spread(expression) => {
                                CallArgument::Spread(self.materialize_global_expression(expression))
                            }
                        })
                        .collect(),
                }
            }
            Expression::New { callee, arguments } => Expression::New {
                callee: Box::new(self.materialize_global_expression(callee)),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => {
                            CallArgument::Expression(self.materialize_global_expression(expression))
                        }
                        CallArgument::Spread(expression) => {
                            CallArgument::Spread(self.materialize_global_expression(expression))
                        }
                    })
                    .collect(),
            },
            _ => expression.clone(),
        }
    }

    fn infer_static_call_result_expression(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Option<Expression> {
        let Expression::Identifier(_) = callee else {
            return None;
        };
        let user_function = match self.infer_global_function_binding(callee)? {
            LocalFunctionBinding::User(function_name) => {
                self.user_function_map.get(&function_name)?
            }
            LocalFunctionBinding::Builtin(_) => return None,
        };
        if user_function.is_async() {
            return None;
        }

        let summary = user_function.inline_summary.as_ref()?;
        if !summary.effects.is_empty() {
            return None;
        }
        let return_value = summary.return_value.as_ref()?;
        Some(self.substitute_global_user_function_argument_bindings(
            return_value,
            user_function,
            arguments,
        ))
    }

    fn substitute_global_user_function_argument_bindings(
        &self,
        expression: &Expression,
        user_function: &UserFunction,
        arguments: &[CallArgument],
    ) -> Expression {
        let mut bindings = HashMap::new();
        for (index, param_name) in user_function.params.iter().enumerate() {
            let value = match arguments.get(index) {
                Some(CallArgument::Expression(expression))
                | Some(CallArgument::Spread(expression)) => expression.clone(),
                None => Expression::Undefined,
            };
            bindings.insert(param_name.clone(), value);
        }
        self.substitute_global_expression_bindings(expression, &bindings)
    }

    fn substitute_global_expression_bindings(
        &self,
        expression: &Expression,
        bindings: &HashMap<String, Expression>,
    ) -> Expression {
        match expression {
            Expression::Identifier(name) => bindings
                .get(name)
                .cloned()
                .unwrap_or_else(|| expression.clone()),
            Expression::Member { object, property } => Expression::Member {
                object: Box::new(self.substitute_global_expression_bindings(object, bindings)),
                property: Box::new(self.substitute_global_expression_bindings(property, bindings)),
            },
            Expression::Assign { name, value } => Expression::Assign {
                name: name.clone(),
                value: Box::new(self.substitute_global_expression_bindings(value, bindings)),
            },
            Expression::AssignMember {
                object,
                property,
                value,
            } => Expression::AssignMember {
                object: Box::new(self.substitute_global_expression_bindings(object, bindings)),
                property: Box::new(self.substitute_global_expression_bindings(property, bindings)),
                value: Box::new(self.substitute_global_expression_bindings(value, bindings)),
            },
            Expression::Unary { op, expression } => Expression::Unary {
                op: *op,
                expression: Box::new(
                    self.substitute_global_expression_bindings(expression, bindings),
                ),
            },
            Expression::Binary { op, left, right } => Expression::Binary {
                op: *op,
                left: Box::new(self.substitute_global_expression_bindings(left, bindings)),
                right: Box::new(self.substitute_global_expression_bindings(right, bindings)),
            },
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => Expression::Conditional {
                condition: Box::new(
                    self.substitute_global_expression_bindings(condition, bindings),
                ),
                then_expression: Box::new(
                    self.substitute_global_expression_bindings(then_expression, bindings),
                ),
                else_expression: Box::new(
                    self.substitute_global_expression_bindings(else_expression, bindings),
                ),
            },
            Expression::Sequence(expressions) => Expression::Sequence(
                expressions
                    .iter()
                    .map(|expression| {
                        self.substitute_global_expression_bindings(expression, bindings)
                    })
                    .collect(),
            ),
            Expression::Array(elements) => Expression::Array(
                elements
                    .iter()
                    .map(|element| match element {
                        crate::ir::hir::ArrayElement::Expression(expression) => {
                            crate::ir::hir::ArrayElement::Expression(
                                self.substitute_global_expression_bindings(expression, bindings),
                            )
                        }
                        crate::ir::hir::ArrayElement::Spread(expression) => {
                            crate::ir::hir::ArrayElement::Spread(
                                self.substitute_global_expression_bindings(expression, bindings),
                            )
                        }
                    })
                    .collect(),
            ),
            Expression::Object(entries) => Expression::Object(
                entries
                    .iter()
                    .map(|entry| match entry {
                        crate::ir::hir::ObjectEntry::Data { key, value } => {
                            crate::ir::hir::ObjectEntry::Data {
                                key: self.substitute_global_expression_bindings(key, bindings),
                                value: self.substitute_global_expression_bindings(value, bindings),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                            crate::ir::hir::ObjectEntry::Getter {
                                key: self.substitute_global_expression_bindings(key, bindings),
                                getter: self
                                    .substitute_global_expression_bindings(getter, bindings),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                            crate::ir::hir::ObjectEntry::Setter {
                                key: self.substitute_global_expression_bindings(key, bindings),
                                setter: self
                                    .substitute_global_expression_bindings(setter, bindings),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Spread(expression) => {
                            crate::ir::hir::ObjectEntry::Spread(
                                self.substitute_global_expression_bindings(expression, bindings),
                            )
                        }
                    })
                    .collect(),
            ),
            Expression::Call { callee, arguments } => Expression::Call {
                callee: Box::new(self.substitute_global_expression_bindings(callee, bindings)),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => CallArgument::Expression(
                            self.substitute_global_expression_bindings(expression, bindings),
                        ),
                        CallArgument::Spread(expression) => CallArgument::Spread(
                            self.substitute_global_expression_bindings(expression, bindings),
                        ),
                    })
                    .collect(),
            },
            Expression::New { callee, arguments } => Expression::New {
                callee: Box::new(self.substitute_global_expression_bindings(callee, bindings)),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => CallArgument::Expression(
                            self.substitute_global_expression_bindings(expression, bindings),
                        ),
                        CallArgument::Spread(expression) => CallArgument::Spread(
                            self.substitute_global_expression_bindings(expression, bindings),
                        ),
                    })
                    .collect(),
            },
            _ => expression.clone(),
        }
    }

    fn infer_global_function_binding(
        &self,
        expression: &Expression,
    ) -> Option<LocalFunctionBinding> {
        match expression {
            Expression::Identifier(name) => {
                if let Some(binding) = self.global_function_bindings.get(name) {
                    return Some(binding.clone());
                }
                if is_internal_user_function_identifier(name)
                    && self.user_function_map.contains_key(name)
                {
                    Some(LocalFunctionBinding::User(name.clone()))
                } else if builtin_identifier_kind(name) == Some(StaticValueKind::Function) {
                    Some(LocalFunctionBinding::Builtin(name.clone()))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn update_static_global_assignment_metadata(&mut self, name: &str, value: &Expression) {
        let snapshot_value = self
            .global_value_bindings
            .get(name)
            .map(|snapshot| substitute_self_referential_binding_snapshot(value, name, snapshot))
            .unwrap_or_else(|| value.clone());
        self.global_kinds.insert(
            name.to_string(),
            infer_global_expression_kind(&snapshot_value),
        );
        self.global_value_bindings.insert(
            name.to_string(),
            self.materialize_global_expression(&snapshot_value),
        );
        if let Some(array_binding) = self.infer_global_array_binding(&snapshot_value) {
            self.global_array_bindings
                .insert(name.to_string(), array_binding);
        } else {
            self.global_array_bindings.remove(name);
        }
        if let Some(object_binding) = self.infer_global_object_binding(&snapshot_value) {
            self.global_object_bindings
                .insert(name.to_string(), object_binding);
        } else {
            self.global_object_bindings.remove(name);
        }
        if let Some(arguments_binding) = self.infer_global_arguments_binding(&snapshot_value) {
            self.global_arguments_bindings
                .insert(name.to_string(), arguments_binding);
        } else {
            self.global_arguments_bindings.remove(name);
        }
        if let Some(function_binding) = self.infer_global_function_binding(&snapshot_value) {
            self.global_function_bindings
                .insert(name.to_string(), function_binding);
            self.global_kinds
                .insert(name.to_string(), StaticValueKind::Function);
        } else {
            self.global_function_bindings.remove(name);
        }
        self.update_global_object_literal_member_bindings_for_value(name, &snapshot_value);
        self.update_global_object_literal_home_bindings(name, &snapshot_value);
        self.update_global_object_prototype_binding_from_value(name, &snapshot_value);
    }

    fn global_member_function_binding_property(
        &self,
        property: &Expression,
    ) -> Option<MemberFunctionBindingProperty> {
        let materialized = self.materialize_global_expression(property);
        if let Some(property_name) = static_property_name_from_expression(&materialized) {
            return Some(MemberFunctionBindingProperty::String(property_name));
        }
        match &materialized {
            Expression::Member { object, property }
                if matches!(object.as_ref(), Expression::Identifier(name) if name == "Symbol")
                    && matches!(property.as_ref(), Expression::String(_)) =>
            {
                let Expression::String(symbol_name) = property.as_ref() else {
                    unreachable!("filtered above");
                };
                Some(MemberFunctionBindingProperty::Symbol(format!(
                    "Symbol.{symbol_name}"
                )))
            }
            Expression::Call { callee, .. } if matches!(callee.as_ref(), Expression::Identifier(name) if name == "Symbol") => {
                Some(MemberFunctionBindingProperty::SymbolExpression(format!(
                    "{materialized:?}"
                )))
            }
            _ => None,
        }
    }

    fn global_member_function_binding_key(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<MemberFunctionBindingKey> {
        let target = match object {
            Expression::Identifier(name) => MemberFunctionBindingTarget::Identifier(name.clone()),
            Expression::Member {
                object,
                property: target_property,
            } if matches!(target_property.as_ref(), Expression::String(name) if name == "prototype") =>
            {
                let Expression::Identifier(name) = object.as_ref() else {
                    return None;
                };
                MemberFunctionBindingTarget::Prototype(name.clone())
            }
            _ => return None,
        };
        let property = self.global_member_function_binding_property(property)?;
        Some(MemberFunctionBindingKey { target, property })
    }

    fn update_global_member_assignment_metadata(
        &mut self,
        object: &Expression,
        property: &Expression,
        value: &Expression,
    ) {
        let materialized_property = self.materialize_global_expression(property);
        let materialized_value = self.materialize_global_expression(value);
        match object {
            Expression::Identifier(name) if self.global_bindings.contains_key(name) => {
                let object_binding = self
                    .global_object_bindings
                    .entry(name.clone())
                    .or_insert_with(empty_object_value_binding);
                object_binding_set_property(
                    object_binding,
                    materialized_property.clone(),
                    materialized_value.clone(),
                );
            }
            Expression::Member {
                object: prototype_object,
                property: target_property,
            } if matches!(target_property.as_ref(), Expression::String(name) if name == "prototype") =>
            {
                let Expression::Identifier(name) = prototype_object.as_ref() else {
                    return;
                };
                let object_binding = self
                    .global_prototype_object_bindings
                    .entry(name.clone())
                    .or_insert_with(empty_object_value_binding);
                object_binding_set_property(
                    object_binding,
                    materialized_property.clone(),
                    materialized_value.clone(),
                );
            }
            _ => {}
        }

        let Some(key) = self.global_member_function_binding_key(object, property) else {
            return;
        };
        if let Some(binding) = self.infer_global_function_binding(value) {
            self.global_member_function_bindings
                .insert(key.clone(), binding);
        } else {
            self.global_member_function_bindings.remove(&key);
        }
        self.global_member_getter_bindings.remove(&key);
        self.global_member_setter_bindings.remove(&key);
    }

    fn register_global_bindings(&mut self, statements: &[Statement]) {
        let mut next_global_index = self.next_allocated_global_index();

        for statement in statements {
            match statement {
                Statement::Var { name, value } => {
                    if !self.global_bindings.contains_key(name) {
                        self.global_bindings.insert(name.clone(), next_global_index);
                        next_global_index += 1;
                    }
                    if !self.global_kinds.contains_key(name) {
                        self.global_kinds
                            .insert(name.clone(), infer_global_expression_kind(value));
                    }
                    let descriptor_value = self.materialize_global_expression(value);
                    match self.global_property_descriptors.get_mut(name) {
                        Some(state) => state.value = descriptor_value,
                        None => {
                            self.global_property_descriptors.insert(
                                name.clone(),
                                GlobalPropertyDescriptorState {
                                    value: descriptor_value,
                                    writable: Some(true),
                                    enumerable: true,
                                    configurable: false,
                                },
                            );
                        }
                    }
                    self.update_static_global_assignment_metadata(name, value);
                }
                Statement::Let { name, value, .. } => {
                    if !self.global_bindings.contains_key(name) {
                        self.global_bindings.insert(name.clone(), next_global_index);
                        next_global_index += 1;
                    }
                    self.global_lexical_bindings.insert(name.clone());
                    if !self.global_kinds.contains_key(name) {
                        self.global_kinds
                            .insert(name.clone(), infer_global_expression_kind(value));
                    }
                    self.update_static_global_assignment_metadata(name, value);
                }
                Statement::Assign { name, value } => {
                    if self.global_bindings.contains_key(name) {
                        self.update_static_global_assignment_metadata(name, value);
                    }
                }
                Statement::AssignMember {
                    object,
                    property,
                    value,
                } => {
                    self.update_global_member_assignment_metadata(object, property, value);
                }
                Statement::Expression(expression) => {
                    self.update_global_expression_metadata(expression);
                }
                _ => {}
            }
        }
    }

    fn register_global_function_bindings(&mut self, functions: &[FunctionDeclaration]) {
        let mut next_global_index = self.next_allocated_global_index();

        for function in functions {
            if !function.register_global {
                continue;
            }

            if !self.global_bindings.contains_key(&function.name) {
                self.global_bindings
                    .insert(function.name.clone(), next_global_index);
                next_global_index += 1;
            }

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

            match self.global_property_descriptors.get_mut(&function.name) {
                Some(state) => {
                    state.value = Expression::Identifier(function.name.clone());
                    state.writable = Some(true);
                    state.enumerable = true;
                    state.configurable = false;
                }
                None => {
                    self.global_property_descriptors.insert(
                        function.name.clone(),
                        GlobalPropertyDescriptorState {
                            value: Expression::Identifier(function.name.clone()),
                            writable: Some(true),
                            enumerable: true,
                            configurable: false,
                        },
                    );
                }
            }
        }
    }

    fn register_user_function_capture_bindings(&mut self, functions: &[FunctionDeclaration]) {
        self.user_function_capture_bindings.clear();

        for function in functions {
            let scope_bindings = collect_function_constructor_local_bindings(function)
                .into_iter()
                .map(|name| {
                    scoped_binding_source_name(&name)
                        .unwrap_or(&name)
                        .to_string()
                })
                .collect::<HashSet<_>>();
            let referenced = collect_referenced_binding_names_from_statements(&function.body);
            let mut captures = HashMap::new();

            for name in referenced {
                let source_name = scoped_binding_source_name(&name)
                    .unwrap_or(&name)
                    .to_string();
                if scope_bindings.contains(&source_name)
                    || self.global_bindings.contains_key(&source_name)
                    || self.user_function_map.contains_key(&source_name)
                    || is_builtin_like_capture_identifier(&source_name)
                {
                    continue;
                }

                let hidden_name =
                    format!("__ayy_capture_binding__{}__{}", function.name, source_name);
                self.ensure_implicit_global_binding(&hidden_name);
                captures.entry(source_name).or_insert(hidden_name);
            }

            if !captures.is_empty() {
                self.user_function_capture_bindings
                    .insert(function.name.clone(), captures);
            }
        }
    }

    fn reserve_function_constructor_implicit_global_bindings(
        &mut self,
        program: &Program,
    ) -> DirectResult<()> {
        let mut names = BTreeSet::new();
        for function in &program.functions {
            if !function.name.starts_with("__ayy_function_ctor_") {
                continue;
            }
            let scope = collect_function_constructor_local_bindings(function);
            collect_implicit_globals_from_statements(
                &function.body,
                function.strict,
                &scope,
                &mut names,
            )?;
        }

        let mut next_global_index = self
            .global_bindings
            .values()
            .copied()
            .chain(
                self.implicit_global_bindings
                    .values()
                    .flat_map(|binding| [binding.value_index, binding.present_index]),
            )
            .max()
            .map(|index| index + 1)
            .unwrap_or(CURRENT_THIS_GLOBAL_INDEX + 1);

        for name in names {
            if self.global_bindings.contains_key(&name)
                || self.implicit_global_bindings.contains_key(&name)
            {
                continue;
            }
            let binding = ImplicitGlobalBinding {
                value_index: next_global_index,
                present_index: next_global_index + 1,
            };
            next_global_index += 2;
            self.implicit_global_bindings.insert(name, binding);
        }

        Ok(())
    }

    fn ensure_implicit_global_binding(&mut self, name: &str) -> ImplicitGlobalBinding {
        if let Some(binding) = self.implicit_global_bindings.get(name).copied() {
            return binding;
        }

        let next_global_index = self
            .global_bindings
            .values()
            .copied()
            .chain(
                self.implicit_global_bindings
                    .values()
                    .flat_map(|binding| [binding.value_index, binding.present_index]),
            )
            .max()
            .map(|index| index + 1)
            .unwrap_or(CURRENT_THIS_GLOBAL_INDEX + 1);

        let binding = ImplicitGlobalBinding {
            value_index: next_global_index,
            present_index: next_global_index + 1,
        };
        self.implicit_global_bindings
            .insert(name.to_string(), binding);
        binding
    }

    fn next_allocated_global_index(&self) -> u32 {
        self.global_bindings
            .values()
            .copied()
            .chain(
                self.implicit_global_bindings
                    .values()
                    .flat_map(|binding| [binding.value_index, binding.present_index]),
            )
            .chain(
                self.global_runtime_prototype_bindings
                    .values()
                    .filter_map(|binding| binding.global_index),
            )
            .max()
            .map(|index| index + 1)
            .unwrap_or(CURRENT_THIS_GLOBAL_INDEX + 1)
    }

    fn reserve_global_runtime_prototype_binding_globals(&mut self) {
        let mut names = self
            .global_runtime_prototype_bindings
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        names.sort();
        let mut next_global_index = self.next_allocated_global_index();
        for name in names {
            if let Some(binding) = self.global_runtime_prototype_bindings.get_mut(&name) {
                binding.global_index = Some(next_global_index);
                next_global_index += 1;
            }
        }
    }

    fn update_user_function_home_object_binding(
        &mut self,
        binding: LocalFunctionBinding,
        home_object_name: &str,
    ) {
        let LocalFunctionBinding::User(function_name) = binding else {
            return;
        };
        if let Some(user_function) = self.user_function_map.get_mut(&function_name) {
            user_function.home_object_binding = Some(home_object_name.to_string());
        }
    }

    fn update_global_object_literal_home_bindings(&mut self, name: &str, value: &Expression) {
        let Expression::Object(entries) = value else {
            return;
        };
        for entry in entries {
            let binding = match entry {
                crate::ir::hir::ObjectEntry::Data { value, .. } => {
                    self.infer_global_function_binding(value)
                }
                crate::ir::hir::ObjectEntry::Getter { getter, .. } => {
                    self.infer_global_function_binding(getter)
                }
                crate::ir::hir::ObjectEntry::Setter { setter, .. } => {
                    self.infer_global_function_binding(setter)
                }
                crate::ir::hir::ObjectEntry::Spread(_) => None,
            };
            if let Some(binding) = binding {
                self.update_user_function_home_object_binding(binding, name);
            }
        }
    }

    fn clear_global_object_literal_member_bindings_for_name(&mut self, name: &str) {
        self.global_member_function_bindings.retain(|key, _| {
            !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) if target == name)
        });
        self.global_member_getter_bindings.retain(|key, _| {
            !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) if target == name)
        });
        self.global_member_setter_bindings.retain(|key, _| {
            !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) if target == name)
        });
    }

    fn update_global_object_literal_member_bindings_for_value(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Expression::Object(entries) = value else {
            self.clear_global_object_literal_member_bindings_for_name(name);
            return;
        };

        self.clear_global_object_literal_member_bindings_for_name(name);

        let mut states: HashMap<
            MemberFunctionBindingProperty,
            (
                Option<LocalFunctionBinding>,
                Option<LocalFunctionBinding>,
                Option<LocalFunctionBinding>,
            ),
        > = HashMap::new();

        for entry in entries {
            let (key, binding, slot) = match entry {
                ObjectEntry::Data { key, value } => {
                    (key, self.infer_global_function_binding(value), 0)
                }
                ObjectEntry::Getter { key, getter } => {
                    (key, self.infer_global_function_binding(getter), 1)
                }
                ObjectEntry::Setter { key, setter } => {
                    (key, self.infer_global_function_binding(setter), 2)
                }
                ObjectEntry::Spread(_) => return,
            };

            let Some(property) = self.infer_global_member_function_binding_property(key) else {
                continue;
            };
            let state = states.entry(property).or_insert((None, None, None));
            match slot {
                0 => {
                    state.0 = binding;
                    state.1 = None;
                    state.2 = None;
                }
                1 => {
                    state.0 = None;
                    state.1 = binding;
                }
                2 => {
                    state.0 = None;
                    state.2 = binding;
                }
                _ => {}
            }
        }

        for (property, (value_binding, getter_binding, setter_binding)) in states {
            let key = MemberFunctionBindingKey {
                target: MemberFunctionBindingTarget::Identifier(name.to_string()),
                property,
            };
            if let Some(binding) = value_binding {
                self.global_member_function_bindings
                    .insert(key.clone(), binding);
            }
            if let Some(binding) = getter_binding {
                self.global_member_getter_bindings
                    .insert(key.clone(), binding);
            }
            if let Some(binding) = setter_binding {
                self.global_member_setter_bindings.insert(key, binding);
            }
        }
    }

    fn update_global_object_prototype_binding(&mut self, name: &str, prototype: &Expression) {
        self.global_object_prototype_bindings.insert(
            name.to_string(),
            self.materialize_global_expression(prototype),
        );
    }

    fn update_global_object_prototype_binding_from_value(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        if let Some(prototype) = object_literal_prototype_expression(value) {
            self.update_global_object_prototype_binding(name, &prototype);
        }
    }

    fn record_global_runtime_prototype_variant(
        &mut self,
        name: &str,
        prototype: Option<&Expression>,
    ) {
        let initial_variant = self.global_object_prototype_bindings.get(name).cloned();
        let prototype = prototype.map(|expression| self.materialize_global_expression(expression));
        let binding = self
            .global_runtime_prototype_bindings
            .entry(name.to_string())
            .or_insert_with(|| GlobalObjectRuntimePrototypeBinding {
                global_index: None,
                variants: vec![initial_variant],
            });
        if !binding
            .variants
            .iter()
            .any(|candidate| *candidate == prototype)
        {
            binding.variants.push(prototype);
        }
    }

    fn update_global_expression_metadata(&mut self, expression: &Expression) {
        match expression {
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.update_global_member_assignment_metadata(object, property, value);
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.update_global_expression_metadata(expression);
                }
            }
            Expression::Call { callee, arguments } => {
                let Expression::Member { object, property } = callee.as_ref() else {
                    return;
                };
                if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Object")
                    || !matches!(property.as_ref(), Expression::String(name) if name == "setPrototypeOf")
                {
                    return;
                }
                let [
                    CallArgument::Expression(Expression::Identifier(target_name)),
                    CallArgument::Expression(prototype),
                    ..,
                ] = arguments.as_slice()
                else {
                    return;
                };
                if !self.global_bindings.contains_key(target_name) {
                    return;
                }
                self.record_global_runtime_prototype_variant(target_name, Some(prototype));
                self.update_global_object_prototype_binding(target_name, prototype);
            }
            _ => {}
        }
    }

    fn compile_start(
        &mut self,
        statements: &[Statement],
        functions: &[FunctionDeclaration],
        strict: bool,
    ) -> DirectResult<CompiledFunction> {
        let mut start_statements = functions
            .iter()
            .filter(|function| function.register_global)
            .map(|function| Statement::Assign {
                name: function.name.clone(),
                value: Expression::Identifier(function.name.clone()),
            })
            .collect::<Vec<_>>();
        start_statements.extend_from_slice(statements);
        FunctionCompiler::new(
            self,
            None,
            false,
            false,
            strict,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        )?
        .compile(&start_statements)
    }

    fn compile_user_function(
        &mut self,
        function: &FunctionDeclaration,
    ) -> DirectResult<CompiledFunction> {
        let Some(user_function) = self.user_function_map.get(&function.name).cloned() else {
            return Ok(CompiledFunction {
                local_count: 0,
                instructions: vec![0x41, 0x00],
            });
        };
        let parameter_bindings = self
            .user_function_parameter_bindings
            .get(&function.name)
            .cloned()
            .unwrap_or_default();
        let parameter_value_bindings = self
            .user_function_parameter_value_bindings
            .get(&function.name)
            .cloned()
            .unwrap_or_default();
        let parameter_array_bindings = self
            .user_function_parameter_array_bindings
            .get(&function.name)
            .cloned()
            .unwrap_or_default();
        let parameter_object_bindings = self
            .user_function_parameter_object_bindings
            .get(&function.name)
            .cloned()
            .unwrap_or_default();
        FunctionCompiler::new(
            self,
            Some(&user_function),
            true,
            function.mapped_arguments,
            function.strict,
            &parameter_bindings,
            &parameter_value_bindings,
            &parameter_array_bindings,
            &parameter_object_bindings,
        )?
        .compile(&function.body)
    }

    fn collect_user_function_parameter_bindings(
        &self,
        program: &Program,
    ) -> (
        HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        HashMap<String, HashMap<String, Option<Expression>>>,
        HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        let value_bindings = self.collect_user_function_parameter_value_bindings(program);
        let mut bindings = HashMap::new();
        let mut array_bindings = HashMap::new();
        let mut object_bindings = HashMap::new();
        for function in &program.functions {
            bindings.insert(function.name.clone(), HashMap::new());
            array_bindings.insert(function.name.clone(), HashMap::new());
            object_bindings.insert(function.name.clone(), HashMap::new());
        }
        let mut top_level_aliases = HashMap::new();
        let mut top_level_value_bindings = self.global_value_bindings.clone();
        let mut top_level_object_state = self.global_object_bindings.clone();
        for statement in &program.statements {
            let aliases_before_statement = top_level_aliases.clone();
            let value_bindings_before_statement = top_level_value_bindings.clone();
            let object_state_before_statement = top_level_object_state.clone();
            self.collect_parameter_bindings_from_statement(
                statement,
                &mut top_level_aliases,
                &mut bindings,
                &mut array_bindings,
                &mut object_bindings,
            );
            self.collect_stateful_callback_bindings_from_statement(
                statement,
                &aliases_before_statement,
                &mut bindings,
                &mut array_bindings,
                &mut object_bindings,
                &value_bindings_before_statement,
                &object_state_before_statement,
                true,
            );
            self.update_parameter_binding_state_from_statement(
                statement,
                &mut top_level_value_bindings,
                &mut top_level_object_state,
            );
        }
        for function in &program.functions {
            let mut aliases = top_level_aliases.clone();
            for parameter in &function.params {
                aliases.entry(parameter.name.clone()).or_insert(None);
            }
            self.collect_parameter_bindings_from_statements(
                &function.body,
                &mut aliases,
                &mut bindings,
                &mut array_bindings,
                &mut object_bindings,
            );
        }

        (bindings, value_bindings, array_bindings, object_bindings)
    }

    fn collect_user_function_parameter_value_bindings(
        &self,
        program: &Program,
    ) -> HashMap<String, HashMap<String, Option<Expression>>> {
        let mut bindings = HashMap::new();
        for function in &program.functions {
            bindings.insert(function.name.clone(), HashMap::new());
        }

        let mut top_level_aliases = HashMap::new();
        for statement in &program.statements {
            self.collect_parameter_value_bindings_from_statement(
                statement,
                &mut top_level_aliases,
                &mut bindings,
            );
        }

        for function in &program.functions {
            let mut aliases = top_level_aliases.clone();
            for parameter in &function.params {
                aliases.entry(parameter.name.clone()).or_insert(None);
            }
            self.collect_parameter_value_bindings_from_statements(
                &function.body,
                &mut aliases,
                &mut bindings,
            );
        }

        bindings
    }

    fn collect_parameter_value_bindings_from_statements(
        &self,
        statements: &[Statement],
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<Expression>>>,
    ) {
        for statement in statements {
            self.collect_parameter_value_bindings_from_statement(statement, aliases, bindings);
        }
    }

    fn collect_parameter_value_bindings_from_statement(
        &self,
        statement: &Statement,
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<Expression>>>,
    ) {
        match statement {
            Statement::Block { body }
            | Statement::Labeled { body, .. }
            | Statement::With { body, .. } => {
                self.collect_parameter_value_bindings_from_statements(body, aliases, bindings);
            }
            Statement::Var { name, value } | Statement::Let { name, value, .. } => {
                self.collect_parameter_value_bindings_from_expression(value, aliases, bindings);
                aliases.insert(
                    name.clone(),
                    self.resolve_function_binding_from_expression_with_aliases(value, aliases),
                );
            }
            Statement::Assign { name, value } => {
                self.collect_parameter_value_bindings_from_expression(value, aliases, bindings);
                aliases.insert(
                    name.clone(),
                    self.resolve_function_binding_from_expression_with_aliases(value, aliases),
                );
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                self.collect_parameter_value_bindings_from_expression(object, aliases, bindings);
                self.collect_parameter_value_bindings_from_expression(property, aliases, bindings);
                self.collect_parameter_value_bindings_from_expression(value, aliases, bindings);
            }
            Statement::Print { values } => {
                for value in values {
                    self.collect_parameter_value_bindings_from_expression(value, aliases, bindings);
                }
            }
            Statement::Expression(expression)
            | Statement::Throw(expression)
            | Statement::Return(expression)
            | Statement::Yield { value: expression }
            | Statement::YieldDelegate { value: expression } => {
                self.collect_parameter_value_bindings_from_expression(
                    expression, aliases, bindings,
                );
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_parameter_value_bindings_from_expression(condition, aliases, bindings);
                let baseline_aliases = aliases.clone();
                let mut then_aliases = baseline_aliases.clone();
                let mut else_aliases = baseline_aliases.clone();
                self.collect_parameter_value_bindings_from_statements(
                    then_branch,
                    &mut then_aliases,
                    bindings,
                );
                self.collect_parameter_value_bindings_from_statements(
                    else_branch,
                    &mut else_aliases,
                    bindings,
                );
                *aliases = self
                    .merge_aliases_for_branches(&baseline_aliases, &[&then_aliases, &else_aliases]);
            }
            Statement::Try {
                body,
                catch_binding,
                catch_setup,
                catch_body,
            } => {
                let baseline_aliases = aliases.clone();
                let mut body_aliases = baseline_aliases.clone();
                self.collect_parameter_value_bindings_from_statements(
                    body,
                    &mut body_aliases,
                    bindings,
                );

                let mut catch_aliases = baseline_aliases.clone();
                if let Some(binding) = catch_binding {
                    catch_aliases.insert(binding.clone(), None);
                }
                self.collect_parameter_value_bindings_from_statements(
                    catch_setup,
                    &mut catch_aliases,
                    bindings,
                );
                self.collect_parameter_value_bindings_from_statements(
                    catch_body,
                    &mut catch_aliases,
                    bindings,
                );
                *aliases = self.merge_aliases_for_branches(
                    &baseline_aliases,
                    &[&body_aliases, &catch_aliases],
                );
            }
            Statement::Switch {
                discriminant,
                cases,
                ..
            } => {
                self.collect_parameter_value_bindings_from_expression(
                    discriminant,
                    aliases,
                    bindings,
                );
                let baseline_aliases = aliases.clone();
                let mut branch_aliases = Vec::new();
                for case in cases {
                    let mut case_aliases = baseline_aliases.clone();
                    if let Some(test) = &case.test {
                        self.collect_parameter_value_bindings_from_expression(
                            test,
                            &mut case_aliases,
                            bindings,
                        );
                    }
                    self.collect_parameter_value_bindings_from_statements(
                        &case.body,
                        &mut case_aliases,
                        bindings,
                    );
                    branch_aliases.push(case_aliases);
                }
                let branch_refs = branch_aliases.iter().collect::<Vec<_>>();
                *aliases = self.merge_aliases_for_branches(&baseline_aliases, &branch_refs);
            }
            Statement::For {
                init,
                condition,
                update,
                break_hook,
                body,
                ..
            } => {
                self.collect_parameter_value_bindings_from_statements(init, aliases, bindings);
                if let Some(condition) = condition {
                    self.collect_parameter_value_bindings_from_expression(
                        condition, aliases, bindings,
                    );
                }
                if let Some(update) = update {
                    self.collect_parameter_value_bindings_from_expression(
                        update, aliases, bindings,
                    );
                }
                if let Some(break_hook) = break_hook {
                    self.collect_parameter_value_bindings_from_expression(
                        break_hook, aliases, bindings,
                    );
                }
                let baseline_aliases = aliases.clone();
                let mut body_aliases = baseline_aliases.clone();
                self.collect_parameter_value_bindings_from_statements(
                    body,
                    &mut body_aliases,
                    bindings,
                );
                *aliases = self.merge_aliases_for_optional_body(&baseline_aliases, &body_aliases);
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
                self.collect_parameter_value_bindings_from_expression(condition, aliases, bindings);
                if let Some(break_hook) = break_hook {
                    self.collect_parameter_value_bindings_from_expression(
                        break_hook, aliases, bindings,
                    );
                }
                let baseline_aliases = aliases.clone();
                let mut body_aliases = baseline_aliases.clone();
                self.collect_parameter_value_bindings_from_statements(
                    body,
                    &mut body_aliases,
                    bindings,
                );
                *aliases = self.merge_aliases_for_optional_body(&baseline_aliases, &body_aliases);
            }
            Statement::Break { .. } | Statement::Continue { .. } => {}
        }
    }

    fn collect_parameter_value_bindings_from_expression(
        &self,
        expression: &Expression,
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<Expression>>>,
    ) {
        match expression {
            Expression::Call { callee, arguments } => {
                self.collect_parameter_value_bindings_from_expression(callee, aliases, bindings);
                self.register_parameter_value_bindings_for_call(
                    callee, arguments, aliases, bindings,
                );
                for argument in arguments {
                    let argument = match argument {
                        CallArgument::Expression(argument) | CallArgument::Spread(argument) => {
                            argument
                        }
                    };
                    self.collect_parameter_value_bindings_from_expression(
                        argument, aliases, bindings,
                    );
                }
            }
            Expression::Assign { name, value } => {
                self.collect_parameter_value_bindings_from_expression(value, aliases, bindings);
                aliases.insert(
                    name.clone(),
                    self.resolve_function_binding_from_expression_with_aliases(value, aliases),
                );
            }
            Expression::Member { object, property } => {
                self.collect_parameter_value_bindings_from_expression(object, aliases, bindings);
                self.collect_parameter_value_bindings_from_expression(property, aliases, bindings);
            }
            Expression::SuperMember { property } => {
                self.collect_parameter_value_bindings_from_expression(property, aliases, bindings);
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.collect_parameter_value_bindings_from_expression(object, aliases, bindings);
                self.collect_parameter_value_bindings_from_expression(property, aliases, bindings);
                self.collect_parameter_value_bindings_from_expression(value, aliases, bindings);
            }
            Expression::AssignSuperMember { property, value } => {
                self.collect_parameter_value_bindings_from_expression(property, aliases, bindings);
                self.collect_parameter_value_bindings_from_expression(value, aliases, bindings);
            }
            Expression::Unary { expression, .. }
            | Expression::Await(expression)
            | Expression::EnumerateKeys(expression)
            | Expression::GetIterator(expression)
            | Expression::IteratorClose(expression) => {
                self.collect_parameter_value_bindings_from_expression(
                    expression, aliases, bindings,
                );
            }
            Expression::Array(elements) => {
                for element in elements {
                    let expression = match element {
                        ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                            expression
                        }
                    };
                    self.collect_parameter_value_bindings_from_expression(
                        expression, aliases, bindings,
                    );
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        ObjectEntry::Data { key, value } => {
                            self.collect_parameter_value_bindings_from_expression(
                                key, aliases, bindings,
                            );
                            self.collect_parameter_value_bindings_from_expression(
                                value, aliases, bindings,
                            );
                        }
                        ObjectEntry::Getter { key, getter } => {
                            self.collect_parameter_value_bindings_from_expression(
                                key, aliases, bindings,
                            );
                            self.collect_parameter_value_bindings_from_expression(
                                getter, aliases, bindings,
                            );
                        }
                        ObjectEntry::Setter { key, setter } => {
                            self.collect_parameter_value_bindings_from_expression(
                                key, aliases, bindings,
                            );
                            self.collect_parameter_value_bindings_from_expression(
                                setter, aliases, bindings,
                            );
                        }
                        ObjectEntry::Spread(expression) => {
                            self.collect_parameter_value_bindings_from_expression(
                                expression, aliases, bindings,
                            );
                        }
                    }
                }
            }
            Expression::Binary { left, right, .. } => {
                self.collect_parameter_value_bindings_from_expression(left, aliases, bindings);
                self.collect_parameter_value_bindings_from_expression(right, aliases, bindings);
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.collect_parameter_value_bindings_from_expression(condition, aliases, bindings);
                self.collect_parameter_value_bindings_from_expression(
                    then_expression,
                    aliases,
                    bindings,
                );
                self.collect_parameter_value_bindings_from_expression(
                    else_expression,
                    aliases,
                    bindings,
                );
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.collect_parameter_value_bindings_from_expression(
                        expression, aliases, bindings,
                    );
                }
            }
            Expression::New { callee, arguments } | Expression::SuperCall { callee, arguments } => {
                self.collect_parameter_value_bindings_from_expression(callee, aliases, bindings);
                for argument in arguments {
                    let argument = match argument {
                        CallArgument::Expression(argument) | CallArgument::Spread(argument) => {
                            argument
                        }
                    };
                    self.collect_parameter_value_bindings_from_expression(
                        argument, aliases, bindings,
                    );
                }
            }
            Expression::Update { .. }
            | Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::Identifier(_)
            | Expression::This
            | Expression::Sent
            | Expression::NewTarget => {}
        }
    }

    fn register_parameter_value_bindings_for_call(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<Expression>>>,
    ) {
        let (called_function_name, call_arguments) = match callee {
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "call") =>
            {
                let Some(LocalFunctionBinding::User(called_function_name)) =
                    self.resolve_function_binding_from_expression_with_aliases(object, aliases)
                else {
                    return;
                };
                (
                    called_function_name,
                    expand_static_call_arguments(arguments, &self.global_array_bindings)
                        .into_iter()
                        .skip(1)
                        .collect::<Vec<_>>(),
                )
            }
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "apply") =>
            {
                let Some(LocalFunctionBinding::User(called_function_name)) =
                    self.resolve_function_binding_from_expression_with_aliases(object, aliases)
                else {
                    return;
                };
                let expanded_arguments =
                    expand_static_call_arguments(arguments, &self.global_array_bindings);
                let apply_expression = expanded_arguments
                    .get(1)
                    .cloned()
                    .unwrap_or(Expression::Undefined);
                let Some(call_arguments) =
                    self.expand_apply_parameter_call_arguments_from_expression(&apply_expression)
                else {
                    return;
                };
                (called_function_name, call_arguments)
            }
            _ => {
                let Some(LocalFunctionBinding::User(called_function_name)) =
                    self.resolve_function_binding_from_expression_with_aliases(callee, aliases)
                else {
                    return;
                };
                (
                    called_function_name,
                    expand_static_call_arguments(arguments, &self.global_array_bindings),
                )
            }
        };
        let Some(user_function) = self.user_function_map.get(&called_function_name) else {
            return;
        };
        let Some(parameter_bindings) = bindings.get_mut(&called_function_name) else {
            return;
        };

        for (index, argument) in call_arguments.iter().enumerate() {
            if index >= user_function.params.len() {
                break;
            }
            let param_name = &user_function.params[index];
            let materialized_argument = self
                .infer_global_object_binding(argument)
                .map(|binding| object_binding_to_expression(&binding))
                .unwrap_or_else(|| self.materialize_global_expression(argument));
            match parameter_bindings.get(param_name) {
                Some(None) => {}
                Some(Some(existing)) if *existing == materialized_argument => {}
                Some(Some(_)) => {
                    parameter_bindings.insert(param_name.to_string(), None);
                }
                None => {
                    parameter_bindings.insert(param_name.to_string(), Some(materialized_argument));
                }
            }
        }

        if call_arguments.len() < user_function.params.len() {
            for param_name in user_function.params.iter().skip(call_arguments.len()) {
                parameter_bindings.insert(param_name.to_string(), None);
            }
        }
    }

    fn merge_aliases_for_branches(
        &self,
        baseline: &HashMap<String, Option<LocalFunctionBinding>>,
        branches: &[&HashMap<String, Option<LocalFunctionBinding>>],
    ) -> HashMap<String, Option<LocalFunctionBinding>> {
        let mut merged = baseline.clone();
        for (name, baseline_binding) in baseline {
            for branch in branches {
                if branch.get(name) != Some(baseline_binding) {
                    merged.insert(name.clone(), None);
                    break;
                }
            }
        }
        merged
    }

    fn merge_aliases_for_optional_body(
        &self,
        before_body: &HashMap<String, Option<LocalFunctionBinding>>,
        after_body: &HashMap<String, Option<LocalFunctionBinding>>,
    ) -> HashMap<String, Option<LocalFunctionBinding>> {
        self.merge_aliases_for_branches(before_body, &[before_body, after_body])
    }

    fn collect_parameter_bindings_from_statements(
        &self,
        statements: &[Statement],
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        for statement in statements {
            self.collect_parameter_bindings_from_statement(
                statement,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
            );
        }
    }

    fn collect_parameter_bindings_from_statement(
        &self,
        statement: &Statement,
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        match statement {
            Statement::Block { body } | Statement::Labeled { body, .. } => {
                self.collect_parameter_bindings_from_statements(
                    body,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
            }
            Statement::Var { name, value } | Statement::Let { name, value, .. } => {
                self.collect_parameter_bindings_from_expression(
                    value,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                let function_binding =
                    self.resolve_function_binding_from_expression_with_aliases(value, aliases);
                aliases.insert(name.clone(), function_binding);
            }
            Statement::Assign { name, value } => {
                self.collect_parameter_bindings_from_expression(
                    value,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                let function_binding =
                    self.resolve_function_binding_from_expression_with_aliases(value, aliases);
                aliases.insert(name.clone(), function_binding);
            }
            Statement::Yield { value } => {
                self.collect_parameter_bindings_from_expression(
                    value,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
            }
            Statement::YieldDelegate { value } => {
                self.collect_parameter_bindings_from_expression(
                    value,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                self.collect_parameter_bindings_from_expression(
                    object,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                self.collect_parameter_bindings_from_expression(
                    property,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                self.collect_parameter_bindings_from_expression(
                    value,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
            }
            Statement::Print { values } => {
                for value in values {
                    self.collect_parameter_bindings_from_expression(
                        value,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                    );
                }
            }
            Statement::Expression(expression)
            | Statement::Throw(expression)
            | Statement::Return(expression) => {
                self.collect_parameter_bindings_from_expression(
                    expression,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_parameter_bindings_from_expression(
                    condition,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                let baseline_aliases = aliases.clone();
                let mut then_aliases = baseline_aliases.clone();
                let mut else_aliases = baseline_aliases.clone();
                self.collect_parameter_bindings_from_statements(
                    then_branch,
                    &mut then_aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                self.collect_parameter_bindings_from_statements(
                    else_branch,
                    &mut else_aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                *aliases = self
                    .merge_aliases_for_branches(&baseline_aliases, &[&then_aliases, &else_aliases]);
            }
            Statement::While {
                condition,
                body,
                break_hook,
                ..
            } => {
                let baseline_aliases = aliases.clone();
                let mut loop_aliases = baseline_aliases.clone();
                self.collect_parameter_bindings_from_expression(
                    condition,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                self.collect_parameter_bindings_from_expression(
                    condition,
                    &mut loop_aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                if let Some(break_hook) = break_hook {
                    self.collect_parameter_bindings_from_expression(
                        break_hook,
                        &mut loop_aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                    );
                }
                self.collect_parameter_bindings_from_statements(
                    body,
                    &mut loop_aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                *aliases = self.merge_aliases_for_optional_body(&baseline_aliases, &loop_aliases);
            }
            Statement::DoWhile {
                condition,
                body,
                break_hook,
                ..
            } => {
                let baseline_aliases = aliases.clone();
                let mut loop_aliases = baseline_aliases.clone();
                self.collect_parameter_bindings_from_statements(
                    body,
                    &mut loop_aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                self.collect_parameter_bindings_from_expression(
                    condition,
                    &mut loop_aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                self.collect_parameter_bindings_from_expression(
                    condition,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                if let Some(break_hook) = break_hook {
                    self.collect_parameter_bindings_from_expression(
                        break_hook,
                        &mut loop_aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                    );
                }
                *aliases = self.merge_aliases_for_optional_body(&baseline_aliases, &loop_aliases);
            }
            Statement::For {
                init,
                condition,
                update,
                body,
                break_hook,
                per_iteration_bindings,
                ..
            } => {
                let baseline_aliases = aliases.clone();
                let mut loop_aliases = baseline_aliases.clone();
                for binding in per_iteration_bindings {
                    loop_aliases.insert(binding.clone(), None);
                }
                self.collect_parameter_bindings_from_statements(
                    init,
                    &mut loop_aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                self.collect_parameter_bindings_from_statements(
                    init,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                if let Some(condition) = condition {
                    self.collect_parameter_bindings_from_expression(
                        condition,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                    );
                    self.collect_parameter_bindings_from_expression(
                        condition,
                        &mut loop_aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                    );
                }
                self.collect_parameter_bindings_from_statements(
                    body,
                    &mut loop_aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                if let Some(update) = update {
                    self.collect_parameter_bindings_from_expression(
                        update,
                        &mut loop_aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                    );
                }
                if let Some(break_hook) = break_hook {
                    self.collect_parameter_bindings_from_expression(
                        break_hook,
                        &mut loop_aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                    );
                }
                *aliases = self.merge_aliases_for_optional_body(&aliases, &loop_aliases);
            }
            Statement::With { object, body } => {
                self.collect_parameter_bindings_from_expression(
                    object,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                let baseline_aliases = aliases.clone();
                let mut with_aliases = baseline_aliases.clone();
                self.collect_parameter_bindings_from_statements(
                    body,
                    &mut with_aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                *aliases = self.merge_aliases_for_optional_body(&baseline_aliases, &with_aliases);
            }
            Statement::Try {
                body,
                catch_setup,
                catch_binding,
                catch_body,
                ..
            } => {
                let baseline_aliases = aliases.clone();
                let mut try_aliases = baseline_aliases.clone();
                let mut catch_aliases = baseline_aliases.clone();
                if let Some(catch_binding) = catch_binding {
                    catch_aliases.insert(catch_binding.clone(), None);
                }
                self.collect_parameter_bindings_from_statements(
                    body,
                    &mut try_aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                self.collect_parameter_bindings_from_statements(
                    catch_setup,
                    &mut catch_aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                self.collect_parameter_bindings_from_statements(
                    catch_body,
                    &mut catch_aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                *aliases = self
                    .merge_aliases_for_branches(&baseline_aliases, &[&try_aliases, &catch_aliases]);
            }
            Statement::Switch {
                discriminant,
                cases,
                bindings: case_bindings,
                ..
            } => {
                self.collect_parameter_bindings_from_expression(
                    discriminant,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                let baseline_aliases = aliases.clone();
                let mut merged_aliases = baseline_aliases.clone();
                for binding in case_bindings {
                    merged_aliases.insert(binding.clone(), None);
                }
                for case in cases {
                    let mut case_aliases = merged_aliases.clone();
                    if let Some(test) = &case.test {
                        self.collect_parameter_bindings_from_expression(
                            test,
                            &mut case_aliases,
                            bindings,
                            array_bindings,
                            object_bindings,
                        );
                    }
                    self.collect_parameter_bindings_from_statements(
                        &case.body,
                        &mut case_aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                    );
                    merged_aliases =
                        self.merge_aliases_for_branches(&merged_aliases, &[&case_aliases]);
                }
                *aliases = merged_aliases;
            }
            Statement::Break { .. } | Statement::Continue { .. } => {}
        }
    }

    fn collect_parameter_bindings_from_expression(
        &self,
        expression: &Expression,
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        match expression {
            Expression::Call { callee, arguments } => {
                self.collect_parameter_bindings_from_expression(
                    callee,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                self.register_callback_bindings_for_call(
                    callee,
                    arguments,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(argument) => {
                            self.collect_parameter_bindings_from_expression(
                                argument,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                        }
                        CallArgument::Spread(argument) => {
                            self.collect_parameter_bindings_from_expression(
                                argument,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                        }
                    }
                }
            }
            Expression::Assign { name, value } => {
                self.collect_parameter_bindings_from_expression(
                    value,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                let function_binding =
                    self.resolve_function_binding_from_expression_with_aliases(value, aliases);
                aliases.insert(name.clone(), function_binding);
            }
            Expression::Member { object, property } => {
                self.collect_parameter_bindings_from_expression(
                    object,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                self.collect_parameter_bindings_from_expression(
                    property,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
            }
            Expression::SuperMember { property } => {
                self.collect_parameter_bindings_from_expression(
                    property,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.collect_parameter_bindings_from_expression(
                    object,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                self.collect_parameter_bindings_from_expression(
                    property,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                self.collect_parameter_bindings_from_expression(
                    value,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
            }
            Expression::AssignSuperMember { property, value } => {
                self.collect_parameter_bindings_from_expression(
                    property,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                self.collect_parameter_bindings_from_expression(
                    value,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
            }
            Expression::Unary { expression, .. } => self
                .collect_parameter_bindings_from_expression(
                    expression,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                ),
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        crate::ir::hir::ArrayElement::Expression(expression) => {
                            self.collect_parameter_bindings_from_expression(
                                expression,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                        }
                        crate::ir::hir::ArrayElement::Spread(expression) => {
                            self.collect_parameter_bindings_from_expression(
                                expression,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                        }
                    }
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        crate::ir::hir::ObjectEntry::Data { key, value } => {
                            self.collect_parameter_bindings_from_expression(
                                key,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                            self.collect_parameter_bindings_from_expression(
                                value,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                            self.collect_parameter_bindings_from_expression(
                                key,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                            self.collect_parameter_bindings_from_expression(
                                getter,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                            self.collect_parameter_bindings_from_expression(
                                key,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                            self.collect_parameter_bindings_from_expression(
                                setter,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Spread(expression) => {
                            self.collect_parameter_bindings_from_expression(
                                expression,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                        }
                    }
                }
            }
            Expression::EnumerateKeys(expression)
            | Expression::GetIterator(expression)
            | Expression::IteratorClose(expression)
            | Expression::Await(expression) => {
                self.collect_parameter_bindings_from_expression(
                    expression,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
            }
            Expression::NewTarget => {}
            Expression::Binary { left, right, .. } => {
                self.collect_parameter_bindings_from_expression(
                    left,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                self.collect_parameter_bindings_from_expression(
                    right,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.collect_parameter_bindings_from_expression(
                    condition,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                self.collect_parameter_bindings_from_expression(
                    then_expression,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                self.collect_parameter_bindings_from_expression(
                    else_expression,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.collect_parameter_bindings_from_expression(
                        expression,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                    );
                }
            }
            Expression::New { callee, arguments } => {
                self.collect_parameter_bindings_from_expression(
                    callee,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(argument) => {
                            self.collect_parameter_bindings_from_expression(
                                argument,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                        }
                        CallArgument::Spread(argument) => {
                            self.collect_parameter_bindings_from_expression(
                                argument,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                        }
                    }
                }
            }
            Expression::SuperCall { callee, arguments } => {
                self.collect_parameter_bindings_from_expression(
                    callee,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(argument) => {
                            self.collect_parameter_bindings_from_expression(
                                argument,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                        }
                        CallArgument::Spread(argument) => {
                            self.collect_parameter_bindings_from_expression(
                                argument,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                        }
                    }
                }
            }
            Expression::Update { .. }
            | Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::Identifier(_)
            | Expression::This
            | Expression::Sent => {}
        }
    }

    fn collect_stateful_callback_bindings_from_statement(
        &self,
        statement: &Statement,
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
        value_bindings: &HashMap<String, Expression>,
        object_state: &HashMap<String, ObjectValueBinding>,
        overwrite_existing: bool,
    ) {
        match statement {
            Statement::Block { body } | Statement::Labeled { body, .. } => {
                for statement in body {
                    self.collect_stateful_callback_bindings_from_statement(
                        statement,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
            }
            Statement::Var { value, .. }
            | Statement::Let { value, .. }
            | Statement::Assign { value, .. }
            | Statement::Yield { value }
            | Statement::YieldDelegate { value }
            | Statement::Expression(value)
            | Statement::Throw(value)
            | Statement::Return(value) => self.collect_stateful_callback_bindings_from_expression(
                value,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
                value_bindings,
                object_state,
                overwrite_existing,
            ),
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                self.collect_stateful_callback_bindings_from_expression(
                    object,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                self.collect_stateful_callback_bindings_from_expression(
                    property,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                self.collect_stateful_callback_bindings_from_expression(
                    value,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
            }
            Statement::Print { values } => {
                for value in values {
                    self.collect_stateful_callback_bindings_from_expression(
                        value,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_stateful_callback_bindings_from_expression(
                    condition,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                for statement in then_branch {
                    self.collect_stateful_callback_bindings_from_statement(
                        statement,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
                for statement in else_branch {
                    self.collect_stateful_callback_bindings_from_statement(
                        statement,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
            }
            Statement::While {
                condition,
                body,
                break_hook,
                ..
            }
            | Statement::DoWhile {
                condition,
                body,
                break_hook,
                ..
            } => {
                self.collect_stateful_callback_bindings_from_expression(
                    condition,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                if let Some(break_hook) = break_hook {
                    self.collect_stateful_callback_bindings_from_expression(
                        break_hook,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
                for statement in body {
                    self.collect_stateful_callback_bindings_from_statement(
                        statement,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
            }
            Statement::For {
                init,
                condition,
                update,
                body,
                break_hook,
                ..
            } => {
                for statement in init {
                    self.collect_stateful_callback_bindings_from_statement(
                        statement,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
                if let Some(condition) = condition {
                    self.collect_stateful_callback_bindings_from_expression(
                        condition,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
                if let Some(update) = update {
                    self.collect_stateful_callback_bindings_from_expression(
                        update,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
                if let Some(break_hook) = break_hook {
                    self.collect_stateful_callback_bindings_from_expression(
                        break_hook,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
                for statement in body {
                    self.collect_stateful_callback_bindings_from_statement(
                        statement,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
            }
            Statement::With { object, body } => {
                self.collect_stateful_callback_bindings_from_expression(
                    object,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                for statement in body {
                    self.collect_stateful_callback_bindings_from_statement(
                        statement,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
            }
            Statement::Try {
                body,
                catch_setup,
                catch_body,
                ..
            } => {
                for statement in body {
                    self.collect_stateful_callback_bindings_from_statement(
                        statement,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
                for statement in catch_setup {
                    self.collect_stateful_callback_bindings_from_statement(
                        statement,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
                for statement in catch_body {
                    self.collect_stateful_callback_bindings_from_statement(
                        statement,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
            }
            Statement::Switch {
                discriminant,
                cases,
                ..
            } => {
                self.collect_stateful_callback_bindings_from_expression(
                    discriminant,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                for case in cases {
                    if let Some(test) = &case.test {
                        self.collect_stateful_callback_bindings_from_expression(
                            test,
                            aliases,
                            bindings,
                            array_bindings,
                            object_bindings,
                            value_bindings,
                            object_state,
                            overwrite_existing,
                        );
                    }
                    for statement in &case.body {
                        self.collect_stateful_callback_bindings_from_statement(
                            statement,
                            aliases,
                            bindings,
                            array_bindings,
                            object_bindings,
                            value_bindings,
                            object_state,
                            overwrite_existing,
                        );
                    }
                }
            }
            Statement::Break { .. } | Statement::Continue { .. } => {}
        }
    }

    fn collect_stateful_callback_bindings_from_expression(
        &self,
        expression: &Expression,
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
        value_bindings: &HashMap<String, Expression>,
        object_state: &HashMap<String, ObjectValueBinding>,
        overwrite_existing: bool,
    ) {
        match expression {
            Expression::Call { callee, arguments } => {
                self.collect_stateful_callback_bindings_from_expression(
                    callee,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                self.register_callback_bindings_for_call_with_state(
                    callee,
                    arguments,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                for argument in arguments {
                    let argument = match argument {
                        CallArgument::Expression(argument) | CallArgument::Spread(argument) => {
                            argument
                        }
                    };
                    self.collect_stateful_callback_bindings_from_expression(
                        argument,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
            }
            Expression::Array(elements) => {
                for element in elements {
                    let element = match element {
                        crate::ir::hir::ArrayElement::Expression(element)
                        | crate::ir::hir::ArrayElement::Spread(element) => element,
                    };
                    self.collect_stateful_callback_bindings_from_expression(
                        element,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        crate::ir::hir::ObjectEntry::Data { key, value } => {
                            self.collect_stateful_callback_bindings_from_expression(
                                key,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                                value_bindings,
                                object_state,
                                overwrite_existing,
                            );
                            self.collect_stateful_callback_bindings_from_expression(
                                value,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                                value_bindings,
                                object_state,
                                overwrite_existing,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                            self.collect_stateful_callback_bindings_from_expression(
                                key,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                                value_bindings,
                                object_state,
                                overwrite_existing,
                            );
                            self.collect_stateful_callback_bindings_from_expression(
                                getter,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                                value_bindings,
                                object_state,
                                overwrite_existing,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                            self.collect_stateful_callback_bindings_from_expression(
                                key,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                                value_bindings,
                                object_state,
                                overwrite_existing,
                            );
                            self.collect_stateful_callback_bindings_from_expression(
                                setter,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                                value_bindings,
                                object_state,
                                overwrite_existing,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Spread(value) => {
                            self.collect_stateful_callback_bindings_from_expression(
                                value,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                                value_bindings,
                                object_state,
                                overwrite_existing,
                            );
                        }
                    }
                }
            }
            Expression::Member { object, property }
            | Expression::AssignMember {
                object, property, ..
            } => {
                self.collect_stateful_callback_bindings_from_expression(
                    object,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                self.collect_stateful_callback_bindings_from_expression(
                    property,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
            }
            Expression::SuperMember { property } => {
                self.collect_stateful_callback_bindings_from_expression(
                    property,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
            }
            Expression::Assign { value, .. }
            | Expression::Await(value)
            | Expression::EnumerateKeys(value)
            | Expression::GetIterator(value)
            | Expression::IteratorClose(value)
            | Expression::Unary {
                expression: value, ..
            } => self.collect_stateful_callback_bindings_from_expression(
                value,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
                value_bindings,
                object_state,
                overwrite_existing,
            ),
            Expression::AssignSuperMember { property, value } => {
                self.collect_stateful_callback_bindings_from_expression(
                    property,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                self.collect_stateful_callback_bindings_from_expression(
                    value,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
            }
            Expression::Binary { left, right, .. } => {
                self.collect_stateful_callback_bindings_from_expression(
                    left,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                self.collect_stateful_callback_bindings_from_expression(
                    right,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.collect_stateful_callback_bindings_from_expression(
                    condition,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                self.collect_stateful_callback_bindings_from_expression(
                    then_expression,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                self.collect_stateful_callback_bindings_from_expression(
                    else_expression,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.collect_stateful_callback_bindings_from_expression(
                        expression,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
            }
            Expression::New { callee, arguments } | Expression::SuperCall { callee, arguments } => {
                self.collect_stateful_callback_bindings_from_expression(
                    callee,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                for argument in arguments {
                    let argument = match argument {
                        CallArgument::Expression(argument) | CallArgument::Spread(argument) => {
                            argument
                        }
                    };
                    self.collect_stateful_callback_bindings_from_expression(
                        argument,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
            }
            Expression::Update { .. }
            | Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::Identifier(_)
            | Expression::This
            | Expression::NewTarget
            | Expression::Sent => {}
        }
    }

    fn update_parameter_binding_state_from_statement(
        &self,
        statement: &Statement,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) {
        match statement {
            Statement::Block { body } | Statement::Labeled { body, .. } => {
                for statement in body {
                    self.update_parameter_binding_state_from_statement(
                        statement,
                        value_bindings,
                        object_bindings,
                    );
                }
            }
            Statement::Var { name, value } | Statement::Let { name, value, .. } => {
                let materialized_value = self
                    .materialize_global_expression_with_state(
                        value,
                        &HashMap::new(),
                        value_bindings,
                        object_bindings,
                    )
                    .unwrap_or_else(|| self.materialize_global_expression(value));
                value_bindings.insert(name.clone(), materialized_value.clone());
                if let Some(binding) = self.infer_global_object_binding_with_state(
                    &materialized_value,
                    value_bindings,
                    object_bindings,
                ) {
                    object_bindings.insert(name.clone(), binding);
                } else {
                    object_bindings.remove(name);
                }
            }
            Statement::Assign { name, value } => {
                let materialized_value = self
                    .materialize_global_expression_with_state(
                        value,
                        &HashMap::new(),
                        value_bindings,
                        object_bindings,
                    )
                    .unwrap_or_else(|| self.materialize_global_expression(value));
                value_bindings.insert(name.clone(), materialized_value.clone());
                if let Some(binding) = self.infer_global_object_binding_with_state(
                    &materialized_value,
                    value_bindings,
                    object_bindings,
                ) {
                    object_bindings.insert(name.clone(), binding);
                } else {
                    object_bindings.remove(name);
                }
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                let materialized_property = self
                    .materialize_global_expression_with_state(
                        property,
                        &HashMap::new(),
                        value_bindings,
                        object_bindings,
                    )
                    .unwrap_or_else(|| self.materialize_global_expression(property));
                let materialized_value = self
                    .materialize_global_expression_with_state(
                        value,
                        &HashMap::new(),
                        value_bindings,
                        object_bindings,
                    )
                    .unwrap_or_else(|| self.materialize_global_expression(value));
                let _ = self.assign_global_member_expression_with_state(
                    object,
                    materialized_property,
                    materialized_value,
                    &mut HashMap::new(),
                    value_bindings,
                    object_bindings,
                );
            }
            Statement::Expression(expression) => self
                .update_parameter_binding_state_from_expression(
                    expression,
                    value_bindings,
                    object_bindings,
                ),
            _ => {}
        }
    }

    fn update_parameter_binding_state_from_expression(
        &self,
        expression: &Expression,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) {
        match expression {
            Expression::Assign { name, value } => {
                let materialized_value = self
                    .materialize_global_expression_with_state(
                        value,
                        &HashMap::new(),
                        value_bindings,
                        object_bindings,
                    )
                    .unwrap_or_else(|| self.materialize_global_expression(value));
                value_bindings.insert(name.clone(), materialized_value.clone());
                if let Some(binding) = self.infer_global_object_binding_with_state(
                    &materialized_value,
                    value_bindings,
                    object_bindings,
                ) {
                    object_bindings.insert(name.clone(), binding);
                } else {
                    object_bindings.remove(name);
                }
                return;
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                let materialized_property = self
                    .materialize_global_expression_with_state(
                        property,
                        &HashMap::new(),
                        value_bindings,
                        object_bindings,
                    )
                    .unwrap_or_else(|| self.materialize_global_expression(property));
                let materialized_value = self
                    .materialize_global_expression_with_state(
                        value,
                        &HashMap::new(),
                        value_bindings,
                        object_bindings,
                    )
                    .unwrap_or_else(|| self.materialize_global_expression(value));
                let _ = self.assign_global_member_expression_with_state(
                    object,
                    materialized_property,
                    materialized_value,
                    &mut HashMap::new(),
                    value_bindings,
                    object_bindings,
                );
                return;
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.update_parameter_binding_state_from_expression(
                        expression,
                        value_bindings,
                        object_bindings,
                    );
                }
                return;
            }
            _ => {}
        }

        let Expression::Call { callee, arguments } = expression else {
            return;
        };
        let Expression::Member { object, property } = callee.as_ref() else {
            return;
        };
        if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Object")
            || !matches!(property.as_ref(), Expression::String(name) if name == "defineProperty")
        {
            return;
        }
        let [
            CallArgument::Expression(target),
            CallArgument::Expression(property),
            CallArgument::Expression(descriptor_expression),
            ..,
        ] = arguments.as_slice()
        else {
            return;
        };
        let Some(descriptor) = resolve_property_descriptor_definition(descriptor_expression) else {
            return;
        };
        let Expression::Identifier(name) = target else {
            return;
        };

        let property = self
            .materialize_global_expression_with_state(
                property,
                &HashMap::new(),
                value_bindings,
                object_bindings,
            )
            .unwrap_or_else(|| self.materialize_global_expression(property));
        let property_name = static_property_name_from_expression(&property);
        let existing_value = object_bindings
            .get(name)
            .and_then(|object_binding| object_binding_lookup_value(object_binding, &property))
            .cloned();
        let current_enumerable = property_name.as_ref().is_some_and(|property_name| {
            object_bindings
                .get(name)
                .map(|object_binding| {
                    !object_binding
                        .non_enumerable_string_properties
                        .iter()
                        .any(|hidden_name| hidden_name == property_name)
                })
                .unwrap_or(false)
        });
        let enumerable = descriptor.enumerable.unwrap_or(current_enumerable);
        let value = if descriptor.is_accessor() {
            Expression::Undefined
        } else {
            descriptor
                .value
                .as_ref()
                .map(|expression| {
                    self.materialize_global_expression_with_state(
                        expression,
                        &HashMap::new(),
                        value_bindings,
                        object_bindings,
                    )
                    .unwrap_or_else(|| self.materialize_global_expression(expression))
                })
                .or(existing_value)
                .unwrap_or(Expression::Undefined)
        };
        let object_binding = object_bindings
            .entry(name.clone())
            .or_insert_with(empty_object_value_binding);
        object_binding_define_property(object_binding, property, value, enumerable);
    }

    fn register_callback_bindings_for_call_with_state(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
        value_bindings: &HashMap<String, Expression>,
        object_state: &HashMap<String, ObjectValueBinding>,
        overwrite_existing: bool,
    ) {
        let (called_function_name, call_arguments) = match callee {
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "call") =>
            {
                let Some(LocalFunctionBinding::User(called_function_name)) =
                    self.resolve_function_binding_from_expression_with_aliases(object, aliases)
                else {
                    return;
                };
                (
                    called_function_name,
                    expand_static_call_arguments(arguments, &self.global_array_bindings)
                        .into_iter()
                        .skip(1)
                        .collect::<Vec<_>>(),
                )
            }
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "apply") =>
            {
                let Some(LocalFunctionBinding::User(called_function_name)) =
                    self.resolve_function_binding_from_expression_with_aliases(object, aliases)
                else {
                    return;
                };
                let expanded_arguments =
                    expand_static_call_arguments(arguments, &self.global_array_bindings);
                let apply_expression = expanded_arguments
                    .get(1)
                    .cloned()
                    .unwrap_or(Expression::Undefined);
                let Some(call_arguments) = self
                    .expand_apply_parameter_call_arguments_from_expression_with_state(
                        &apply_expression,
                        value_bindings,
                        object_state,
                    )
                else {
                    return;
                };
                (called_function_name, call_arguments)
            }
            _ => {
                let Some(LocalFunctionBinding::User(called_function_name)) =
                    self.resolve_function_binding_from_expression_with_aliases(callee, aliases)
                else {
                    return;
                };
                (
                    called_function_name,
                    expand_static_call_arguments(arguments, &self.global_array_bindings),
                )
            }
        };
        let Some(user_function) = self.user_function_map.get(&called_function_name) else {
            return;
        };
        let Some(parameter_bindings) = bindings.get_mut(&called_function_name) else {
            return;
        };
        let Some(parameter_array_bindings) = array_bindings.get_mut(&called_function_name) else {
            return;
        };
        let Some(parameter_object_bindings) = object_bindings.get_mut(&called_function_name) else {
            return;
        };

        let mut register_candidate =
            |param_name: &str, candidate: Option<LocalFunctionBinding>| match candidate {
                None => {
                    if overwrite_existing {
                        parameter_bindings.insert(param_name.to_string(), None);
                    } else {
                        parameter_bindings.insert(param_name.to_string(), None);
                    }
                }
                Some(binding) => match parameter_bindings.get(param_name) {
                    Some(None) if !overwrite_existing => {}
                    Some(Some(existing)) if *existing == binding => {}
                    Some(Some(_)) if !overwrite_existing => {
                        parameter_bindings.insert(param_name.to_string(), None);
                    }
                    _ => {
                        parameter_bindings.insert(param_name.to_string(), Some(binding));
                    }
                },
            };
        let mut register_object_candidate =
            |param_name: &str, candidate: Option<ObjectValueBinding>| match candidate {
                None if overwrite_existing => {
                    parameter_object_bindings.insert(param_name.to_string(), None);
                }
                None => {
                    parameter_object_bindings.insert(param_name.to_string(), None);
                }
                Some(binding) => match parameter_object_bindings.get(param_name) {
                    Some(None) if !overwrite_existing => {}
                    Some(Some(existing)) if *existing == binding => {}
                    Some(Some(_)) if !overwrite_existing => {
                        parameter_object_bindings.insert(param_name.to_string(), None);
                    }
                    _ => {
                        parameter_object_bindings.insert(param_name.to_string(), Some(binding));
                    }
                },
            };
        let mut register_array_candidate =
            |param_name: &str, candidate: Option<ArrayValueBinding>| match candidate {
                None => {
                    if overwrite_existing {
                        parameter_array_bindings.insert(param_name.to_string(), None);
                    } else {
                        parameter_array_bindings.insert(param_name.to_string(), None);
                    }
                }
                Some(binding) => match parameter_array_bindings.get(param_name) {
                    Some(None) if !overwrite_existing => {}
                    Some(Some(existing)) if *existing == binding => {}
                    Some(Some(_)) if !overwrite_existing => {
                        parameter_array_bindings.insert(param_name.to_string(), None);
                    }
                    _ => {
                        parameter_array_bindings.insert(param_name.to_string(), Some(binding));
                    }
                },
            };

        for (index, argument) in call_arguments.iter().enumerate() {
            if index >= user_function.params.len() {
                break;
            }
            let param_name = &user_function.params[index];
            register_candidate(
                param_name,
                self.resolve_function_binding_from_expression_with_aliases(argument, aliases),
            );
            let materialized_argument = self
                .materialize_global_expression_with_state(
                    argument,
                    &HashMap::new(),
                    value_bindings,
                    object_state,
                )
                .unwrap_or_else(|| self.materialize_global_expression(argument));
            register_array_candidate(
                param_name,
                self.infer_global_array_binding(&materialized_argument),
            );
            let mut value_state = value_bindings.clone();
            let mut object_state = object_state.clone();
            register_object_candidate(
                param_name,
                self.infer_global_object_binding_with_state(
                    argument,
                    &mut value_state,
                    &mut object_state,
                ),
            );
        }

        if call_arguments.len() < user_function.params.len() {
            for param_name in user_function.params.iter().skip(call_arguments.len()) {
                parameter_bindings.insert(param_name.to_string(), None);
                parameter_array_bindings.insert(param_name.to_string(), None);
                parameter_object_bindings.insert(param_name.to_string(), None);
            }
        }
    }

    fn expand_apply_parameter_call_arguments_from_expression_with_state(
        &self,
        expression: &Expression,
        value_bindings: &HashMap<String, Expression>,
        object_bindings: &HashMap<String, ObjectValueBinding>,
    ) -> Option<Vec<Expression>> {
        let materialized = self
            .materialize_global_expression_with_state(
                expression,
                &HashMap::new(),
                value_bindings,
                object_bindings,
            )
            .unwrap_or_else(|| self.materialize_global_expression(expression));
        match &materialized {
            Expression::Undefined | Expression::Null => Some(Vec::new()),
            Expression::Array(elements) => {
                let mut value_bindings = value_bindings.clone();
                let mut object_bindings = object_bindings.clone();
                let mut values = Vec::new();
                for element in elements {
                    match element {
                        ArrayElement::Expression(expression) => {
                            if let Some(object_binding) = self
                                .infer_global_object_binding_with_state(
                                    expression,
                                    &mut value_bindings,
                                    &mut object_bindings,
                                )
                            {
                                values.push(object_binding_to_expression(&object_binding));
                            } else {
                                values.push(
                                    self.materialize_global_expression_with_state(
                                        expression,
                                        &HashMap::new(),
                                        &value_bindings,
                                        &object_bindings,
                                    )
                                    .unwrap_or_else(|| {
                                        self.materialize_global_expression(expression)
                                    }),
                                );
                            }
                        }
                        ArrayElement::Spread(expression) => {
                            let spread_expression = self
                                .materialize_global_expression_with_state(
                                    expression,
                                    &HashMap::new(),
                                    &value_bindings,
                                    &object_bindings,
                                )
                                .unwrap_or_else(|| self.materialize_global_expression(expression));
                            let array_binding =
                                self.infer_global_array_binding(&spread_expression)?;
                            values.extend(
                                array_binding
                                    .values
                                    .into_iter()
                                    .map(|value| value.unwrap_or(Expression::Undefined)),
                            );
                        }
                    }
                }
                Some(values)
            }
            _ => self.expand_apply_parameter_call_arguments_from_expression(&materialized),
        }
    }

    fn register_callback_bindings_for_call(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        let (called_function_name, call_arguments) = match callee {
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "call") =>
            {
                let Some(LocalFunctionBinding::User(called_function_name)) =
                    self.resolve_function_binding_from_expression_with_aliases(object, aliases)
                else {
                    return;
                };
                (
                    called_function_name,
                    expand_static_call_arguments(arguments, &self.global_array_bindings)
                        .into_iter()
                        .skip(1)
                        .collect::<Vec<_>>(),
                )
            }
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "apply") =>
            {
                let Some(LocalFunctionBinding::User(called_function_name)) =
                    self.resolve_function_binding_from_expression_with_aliases(object, aliases)
                else {
                    return;
                };
                let expanded_arguments =
                    expand_static_call_arguments(arguments, &self.global_array_bindings);
                let apply_expression = expanded_arguments
                    .get(1)
                    .cloned()
                    .unwrap_or(Expression::Undefined);
                let Some(call_arguments) =
                    self.expand_apply_parameter_call_arguments_from_expression(&apply_expression)
                else {
                    return;
                };
                (called_function_name, call_arguments)
            }
            _ => {
                let Some(LocalFunctionBinding::User(called_function_name)) =
                    self.resolve_function_binding_from_expression_with_aliases(callee, aliases)
                else {
                    return;
                };
                (
                    called_function_name,
                    expand_static_call_arguments(arguments, &self.global_array_bindings),
                )
            }
        };
        let Some(user_function) = self.user_function_map.get(&called_function_name) else {
            return;
        };
        let Some(parameter_bindings) = bindings.get_mut(&called_function_name) else {
            return;
        };
        let Some(parameter_array_bindings) = array_bindings.get_mut(&called_function_name) else {
            return;
        };
        let Some(parameter_object_bindings) = object_bindings.get_mut(&called_function_name) else {
            return;
        };

        let mut register_candidate =
            |param_name: &str, candidate: Option<LocalFunctionBinding>| match candidate {
                None => {
                    parameter_bindings.insert(param_name.to_string(), None);
                }
                Some(binding) => match parameter_bindings.get(param_name) {
                    Some(None) => {}
                    Some(Some(existing)) if *existing == binding => {}
                    Some(Some(_)) => {
                        parameter_bindings.insert(param_name.to_string(), None);
                    }
                    None => {
                        parameter_bindings.insert(param_name.to_string(), Some(binding));
                    }
                },
            };
        let mut register_object_candidate =
            |param_name: &str, candidate: Option<ObjectValueBinding>| match candidate {
                None => {
                    parameter_object_bindings.insert(param_name.to_string(), None);
                }
                Some(binding) => match parameter_object_bindings.get(param_name) {
                    Some(None) => {}
                    Some(Some(existing)) if *existing == binding => {}
                    Some(Some(_)) => {
                        parameter_object_bindings.insert(param_name.to_string(), None);
                    }
                    None => {
                        parameter_object_bindings.insert(param_name.to_string(), Some(binding));
                    }
                },
            };
        let mut register_array_candidate =
            |param_name: &str, candidate: Option<ArrayValueBinding>| match candidate {
                None => {
                    parameter_array_bindings.insert(param_name.to_string(), None);
                }
                Some(binding) => match parameter_array_bindings.get(param_name) {
                    Some(None) => {}
                    Some(Some(existing)) if *existing == binding => {}
                    Some(Some(_)) => {
                        parameter_array_bindings.insert(param_name.to_string(), None);
                    }
                    None => {
                        parameter_array_bindings.insert(param_name.to_string(), Some(binding));
                    }
                },
            };

        for (index, argument) in call_arguments.iter().enumerate() {
            if index >= user_function.params.len() {
                break;
            }
            let param_name = &user_function.params[index];
            register_candidate(
                param_name,
                self.resolve_function_binding_from_expression_with_aliases(argument, aliases),
            );
            register_array_candidate(param_name, self.infer_global_array_binding(argument));
            register_object_candidate(param_name, self.infer_global_object_binding(argument));
        }

        if call_arguments.len() < user_function.params.len() {
            for param_name in user_function.params.iter().skip(call_arguments.len()) {
                parameter_bindings.insert(param_name.to_string(), None);
                parameter_array_bindings.insert(param_name.to_string(), None);
                parameter_object_bindings.insert(param_name.to_string(), None);
            }
        }
    }

    fn expand_apply_parameter_call_arguments_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<Vec<Expression>> {
        let materialized = self.materialize_global_expression(expression);
        match &materialized {
            Expression::Undefined | Expression::Null => Some(Vec::new()),
            Expression::Array(elements) => {
                let mut value_bindings = self.global_value_bindings.clone();
                let mut object_bindings = self.global_object_bindings.clone();
                let mut values = Vec::new();
                for element in elements {
                    match element {
                        ArrayElement::Expression(expression) => {
                            if let Some(object_binding) = self
                                .infer_global_object_binding_with_state(
                                    expression,
                                    &mut value_bindings,
                                    &mut object_bindings,
                                )
                            {
                                values.push(object_binding_to_expression(&object_binding));
                            } else {
                                values.push(
                                    self.materialize_global_expression_with_state(
                                        expression,
                                        &HashMap::new(),
                                        &value_bindings,
                                        &object_bindings,
                                    )
                                    .unwrap_or_else(|| {
                                        self.materialize_global_expression(expression)
                                    }),
                                );
                            }
                        }
                        ArrayElement::Spread(expression) => {
                            let spread_expression = self
                                .materialize_global_expression_with_state(
                                    expression,
                                    &HashMap::new(),
                                    &value_bindings,
                                    &object_bindings,
                                )
                                .unwrap_or_else(|| self.materialize_global_expression(expression));
                            let array_binding =
                                self.infer_global_array_binding(&spread_expression)?;
                            values.extend(
                                array_binding
                                    .values
                                    .into_iter()
                                    .map(|value| value.unwrap_or(Expression::Undefined)),
                            );
                        }
                    }
                }
                Some(values)
            }
            _ => {
                if let Some(array_binding) = self.infer_global_array_binding(&materialized) {
                    return Some(
                        array_binding
                            .values
                            .into_iter()
                            .map(|value| value.unwrap_or(Expression::Undefined))
                            .collect(),
                    );
                }
                self.infer_global_arguments_binding(&materialized)
                    .map(|binding| binding.values)
            }
        }
    }

    fn resolve_function_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<LocalFunctionBinding> {
        self.resolve_function_binding_from_expression_with_aliases(expression, &HashMap::new())
    }

    fn resolve_function_binding_from_expression_with_aliases(
        &self,
        expression: &Expression,
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
    ) -> Option<LocalFunctionBinding> {
        match expression {
            Expression::Identifier(name) => {
                if let Some(function_binding) = aliases.get(name) {
                    return function_binding.clone();
                }
                if is_internal_user_function_identifier(name)
                    && self.user_function_map.contains_key(name)
                {
                    Some(LocalFunctionBinding::User(name.clone()))
                } else if let Some(function_binding) = self.global_function_bindings.get(name) {
                    Some(function_binding.clone())
                } else if name == "eval" || infer_call_result_kind(name).is_some() {
                    Some(LocalFunctionBinding::Builtin(name.clone()))
                } else {
                    None
                }
            }
            Expression::Sequence(expressions) => expressions.last().and_then(|expression| {
                self.resolve_function_binding_from_expression_with_aliases(expression, aliases)
            }),
            _ => None,
        }
    }
}

