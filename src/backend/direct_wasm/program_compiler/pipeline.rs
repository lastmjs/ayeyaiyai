use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn compile(
        &mut self,
        program: &Program,
    ) -> DirectResult<Vec<u8>> {
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
        self.global_arrays_with_runtime_state.clear();
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
        self.global_member_function_capture_slots.clear();
        self.global_member_getter_bindings.clear();
        self.global_member_setter_bindings.clear();
        self.eval_local_function_bindings.clear();
        self.user_function_capture_bindings.clear();
        self.user_function_assigned_nonlocal_binding_results.clear();
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

    pub(in crate::backend::direct_wasm) fn allocate_test262_realm(&mut self) -> u32 {
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

    pub(in crate::backend::direct_wasm) fn test262_realm_object_binding(
        &self,
        realm_id: u32,
    ) -> Option<ObjectValueBinding> {
        self.test262_realms.get(&realm_id)?;
        let mut realm_object_binding = empty_object_value_binding();
        object_binding_set_property(
            &mut realm_object_binding,
            Expression::String("global".to_string()),
            Expression::Identifier(test262_realm_global_identifier(realm_id)),
        );
        Some(realm_object_binding)
    }

    pub(in crate::backend::direct_wasm) fn register_functions(
        &mut self,
        functions: &[FunctionDeclaration],
    ) -> DirectResult<()> {
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

    pub(in crate::backend::direct_wasm) fn register_returned_function_object_bindings(
        &mut self,
        function: &FunctionDeclaration,
    ) {
        let Some(Expression::Identifier(returned_function_name)) =
            collect_returned_identifier_source_expression(&function.body)
        else {
            return;
        };
        if !self.user_function_map.contains_key(&returned_function_name) {
            return;
        }
        let Some(returned_member_value_bindings) = self
            .user_function_map
            .get(&function.name)
            .map(|user_function| user_function.returned_member_value_bindings.clone())
        else {
            return;
        };
        if returned_member_value_bindings.is_empty() {
            return;
        }
        let object_binding = self
            .global_object_bindings
            .entry(returned_function_name)
            .or_insert_with(empty_object_value_binding);
        for binding in &returned_member_value_bindings {
            object_binding_set_property(
                object_binding,
                Expression::String(binding.property.clone()),
                binding.value.clone(),
            );
        }
    }

    pub(in crate::backend::direct_wasm) fn compile_start(
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

    pub(in crate::backend::direct_wasm) fn compile_user_function(
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
        let function_compiler = FunctionCompiler::new(
            self,
            Some(&user_function),
            true,
            function.mapped_arguments,
            function.strict,
            &parameter_bindings,
            &parameter_value_bindings,
            &parameter_array_bindings,
            &parameter_object_bindings,
        )?;
        let assigned_nonlocal_bindings =
            function_compiler.collect_user_function_assigned_nonlocal_bindings(&user_function);
        let compiled = function_compiler.compile(&function.body)?;
        let assigned_nonlocal_binding_results = assigned_nonlocal_bindings
            .into_iter()
            .filter(|name| {
                self.global_bindings.contains_key(name)
                    || self.implicit_global_bindings.contains_key(name)
            })
            .map(|name| {
                (
                    name.clone(),
                    self.global_value_bindings
                        .get(&name)
                        .cloned()
                        .unwrap_or(Expression::Undefined),
                )
            })
            .collect::<HashMap<_, _>>();
        if !assigned_nonlocal_binding_results.is_empty() {
            self.user_function_assigned_nonlocal_binding_results.insert(
                user_function.name.clone(),
                assigned_nonlocal_binding_results,
            );
        }
        Ok(compiled)
    }
}
