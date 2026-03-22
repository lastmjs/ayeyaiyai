use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_eval_comment_pattern(
        &mut self,
        argument: &Expression,
    ) -> DirectResult<bool> {
        if !self.top_level_function {
            return Ok(false);
        }

        let mut fragments = Vec::new();
        if !self.collect_string_concat_fragments(argument, &mut fragments) {
            return Ok(false);
        }

        let [
            StringConcatFragment::Static(prefix),
            StringConcatFragment::Dynamic(inserted),
            StringConcatFragment::Static(suffix),
        ] = fragments.as_slice()
        else {
            return Ok(false);
        };

        if prefix == "/*var "
            && suffix == "xx = 1*/"
            && self.resolve_single_char_code_expression(inserted).is_some()
        {
            if let Some(code_expression) = self.resolve_single_char_code_expression(inserted) {
                self.emit_numeric_expression(&code_expression)?;
                self.instructions.push(0x1a);
            }
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(true);
        }

        if prefix == "//var " && suffix == "yy = -1" {
            let Some(code_expression) = self.resolve_single_char_code_expression(inserted) else {
                return Ok(false);
            };

            self.emit_line_terminator_check(&code_expression)?;
            self.instructions.push(0x04);
            self.instructions.push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.emit_statement(&Statement::Assign {
                name: "yy".to_string(),
                value: Expression::Unary {
                    op: UnaryOp::Negate,
                    expression: Box::new(Expression::Number(1.0)),
                },
            })?;
            self.instructions.push(0x0b);
            self.pop_control_frame();
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(true);
        }

        Ok(false)
    }

    pub(in crate::backend::direct_wasm) fn emit_builtin_call_for_callee(
        &mut self,
        callee: &Expression,
        name: &str,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if name == "eval" {
            if matches!(callee, Expression::Identifier(identifier) if identifier == "eval") {
                return self.emit_eval_call(arguments);
            }
            return self.emit_indirect_eval_call(arguments);
        }

        self.emit_builtin_call(name, arguments)
    }

    pub(in crate::backend::direct_wasm) fn emit_line_terminator_check(
        &mut self,
        code_expression: &Expression,
    ) -> DirectResult<()> {
        let code_local = self.allocate_temp_local();
        self.emit_numeric_expression(code_expression)?;
        self.push_local_set(code_local);

        let line_terminators = [0x000A, 0x000D, 0x2028, 0x2029];
        let mut first = true;
        for line_terminator in line_terminators {
            self.push_local_get(code_local);
            self.push_i32_const(line_terminator);
            self.push_binary_op(BinaryOp::Equal)?;
            if !first {
                self.push_binary_op(BinaryOp::BitwiseOr)?;
            }
            first = false;
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn resolve_single_char_code_expression(
        &self,
        expression: &Expression,
    ) -> Option<Expression> {
        let resolved = self.resolve_bound_alias_expression(expression)?;
        let Expression::Call { callee, arguments } = resolved else {
            return None;
        };
        let Expression::Member { object, property } = callee.as_ref() else {
            return None;
        };
        if !matches!(object.as_ref(), Expression::Identifier(name) if name == "String") {
            return None;
        }
        if !matches!(property.as_ref(), Expression::String(name) if name == "fromCharCode") {
            return None;
        }
        let [CallArgument::Expression(argument)] = arguments.as_slice() else {
            return None;
        };
        self.resolve_char_code_argument(argument)
    }

    pub(in crate::backend::direct_wasm) fn resolve_char_code_argument(
        &self,
        argument: &Expression,
    ) -> Option<Expression> {
        if let Some(resolved) = self.resolve_bound_alias_expression(argument) {
            if resolved != *argument {
                return self.resolve_char_code_argument(&resolved);
            }
        }

        match argument {
            Expression::Number(_) | Expression::Identifier(_) => {
                Some(self.materialize_static_expression(argument))
            }
            Expression::Binary {
                op: BinaryOp::Add,
                left,
                right,
            } if matches!(left.as_ref(), Expression::String(prefix) if prefix == "0x") => {
                self.resolve_hex_quad_numeric_expression(right)
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_hex_quad_numeric_expression(
        &self,
        expression: &Expression,
    ) -> Option<Expression> {
        let mut digits = Vec::new();
        if !self.collect_hex_digit_expressions(expression, &mut digits) || digits.len() != 4 {
            return None;
        }

        let mut combined = digits[0].clone();
        for digit in digits.into_iter().skip(1) {
            combined = Expression::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expression::Binary {
                    op: BinaryOp::LeftShift,
                    left: Box::new(combined),
                    right: Box::new(Expression::Number(4.0)),
                }),
                right: Box::new(digit),
            };
        }
        Some(combined)
    }

    pub(in crate::backend::direct_wasm) fn collect_hex_digit_expressions(
        &self,
        expression: &Expression,
        digits: &mut Vec<Expression>,
    ) -> bool {
        if let Some(resolved) = self.resolve_bound_alias_expression(expression) {
            if !static_expression_matches(&resolved, expression) {
                return self.collect_hex_digit_expressions(&resolved, digits);
            }
        }

        match expression {
            Expression::Binary {
                op: BinaryOp::Add,
                left,
                right,
            } => {
                self.collect_hex_digit_expressions(left, digits)
                    && self.collect_hex_digit_expressions(right, digits)
            }
            Expression::String(text) if text.len() == 1 => {
                let Some(digit) = text.chars().next().and_then(hex_digit_value) else {
                    return false;
                };
                digits.push(Expression::Number(digit as f64));
                true
            }
            Expression::Member { object, property } => {
                let Some(array_binding) = self.resolve_array_binding_from_expression(object) else {
                    return false;
                };
                if !is_canonical_hex_digit_array(&array_binding) {
                    return false;
                }
                digits.push(self.materialize_static_expression(property));
                true
            }
            _ => false,
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_eval_call(
        &mut self,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let Some(argument) = arguments.first() else {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(true);
        };

        let emit_argument_discard =
            |compiler: &mut Self, argument: &CallArgument| -> DirectResult<()> {
                match argument {
                    CallArgument::Expression(expression) => {
                        compiler.emit_numeric_expression(expression)?;
                        compiler.instructions.push(0x1a);
                    }
                    CallArgument::Spread(expression) => {
                        compiler.emit_numeric_expression(expression)?;
                        compiler.instructions.push(0x1a);
                    }
                }
                Ok(())
            };

        match argument {
            CallArgument::Expression(expression)
                if self.emit_eval_comment_pattern(expression)? =>
            {
                for argument in arguments.iter().skip(1) {
                    emit_argument_discard(self, argument)?;
                }
                Ok(true)
            }
            CallArgument::Expression(Expression::String(argument_source)) => {
                let argument_source = if self.strict_mode {
                    let mut strict_argument_source = String::from("\"use strict\";");
                    strict_argument_source.push_str(argument_source);
                    Cow::Owned(strict_argument_source)
                } else {
                    Cow::Borrowed(argument_source.as_str())
                };

                for argument in arguments.iter().skip(1) {
                    emit_argument_discard(self, argument)?;
                }

                let mut program = if let Some(program) =
                    self.parse_eval_program_in_current_function_context(&argument_source)
                {
                    program
                } else if let Ok(program) = frontend::parse_script_goal(&argument_source) {
                    program
                } else {
                    self.emit_named_error_throw("SyntaxError")?;
                    return Ok(true);
                };

                if self.eval_arguments_declaration_conflicts(&program) {
                    self.emit_named_error_throw("SyntaxError")?;
                    return Ok(true);
                }

                if self.eval_program_declares_var_collision_with_global_lexical(&program) {
                    self.emit_named_error_throw("SyntaxError")?;
                    return Ok(true);
                }

                if self.eval_program_declares_var_collision_with_active_lexical(&program) {
                    self.emit_named_error_throw("SyntaxError")?;
                    return Ok(true);
                }

                if self.eval_program_declares_non_definable_global_function(&program) {
                    self.emit_named_error_throw("TypeError")?;
                    return Ok(true);
                }

                let preexisting_locals = self.locals.keys().cloned().collect::<HashSet<_>>();
                let eval_local_function_declarations = if program.strict {
                    HashMap::new()
                } else {
                    collect_eval_local_function_declarations(
                        &program.statements,
                        &program
                            .functions
                            .iter()
                            .filter(|function| is_eval_local_function_candidate(function))
                            .map(|function| function.name.clone())
                            .collect::<HashSet<_>>(),
                    )
                };
                self.prepare_eval_lexical_bindings(
                    &mut program.statements,
                    &eval_local_function_declarations,
                )?;
                self.prepare_eval_var_bindings(&mut program.statements, program.strict)?;
                self.register_bindings_skipping_eval_local_function_declarations(
                    &program.statements,
                    &eval_local_function_declarations,
                )?;
                self.instantiate_eval_var_bindings(&program, &preexisting_locals)?;
                self.instantiate_eval_global_functions(&program.functions)?;
                self.instantiate_eval_local_functions(&eval_local_function_declarations)?;

                let previous_strict_mode = self.strict_mode;
                self.strict_mode = program.strict;

                let emit_result = self.with_active_eval_lexical_scope(
                    collect_direct_eval_lexical_binding_names(&program.statements),
                    |compiler| {
                        let eval_statements = program
                            .statements
                            .iter()
                            .filter(|statement| {
                                !is_eval_local_function_declaration_statement(
                                    statement,
                                    &eval_local_function_declarations,
                                )
                            })
                            .collect::<Vec<_>>();

                        if let Some((last, rest)) = eval_statements.split_last() {
                            for statement in rest {
                                compiler.emit_statement(statement)?;
                            }

                            match *last {
                                Statement::Expression(expression) => {
                                    compiler.emit_numeric_expression(expression)?
                                }
                                Statement::Assign { name, value } => compiler
                                    .emit_numeric_expression(&Expression::Assign {
                                        name: name.clone(),
                                        value: Box::new(value.clone()),
                                    })?,
                                Statement::AssignMember {
                                    object,
                                    property,
                                    value,
                                } => {
                                    compiler.emit_numeric_expression(&Expression::AssignMember {
                                        object: Box::new(object.clone()),
                                        property: Box::new(property.clone()),
                                        value: Box::new(value.clone()),
                                    })?
                                }
                                _ => {
                                    compiler.emit_statement(last)?;
                                    compiler.push_i32_const(JS_UNDEFINED_TAG);
                                }
                            }
                        } else {
                            compiler.push_i32_const(JS_UNDEFINED_TAG);
                        }

                        Ok(())
                    },
                );

                self.strict_mode = previous_strict_mode;
                emit_result?;

                Ok(true)
            }
            _ => {
                match argument {
                    CallArgument::Expression(expression) => {
                        self.emit_numeric_expression(expression)?
                    }
                    CallArgument::Spread(expression) => {
                        self.emit_numeric_expression(expression)?;
                        self.instructions.push(0x1a);
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                }

                for argument in arguments.iter().skip(1) {
                    emit_argument_discard(self, argument)?;
                }

                Ok(true)
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_indirect_eval_call(
        &mut self,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let Some(argument) = arguments.first() else {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(true);
        };

        let emit_argument_discard =
            |compiler: &mut Self, argument: &CallArgument| -> DirectResult<()> {
                match argument {
                    CallArgument::Expression(expression) => {
                        compiler.emit_numeric_expression(expression)?;
                        compiler.instructions.push(0x1a);
                    }
                    CallArgument::Spread(expression) => {
                        compiler.emit_numeric_expression(expression)?;
                        compiler.instructions.push(0x1a);
                    }
                }
                Ok(())
            };

        match argument {
            CallArgument::Expression(Expression::String(argument_source)) => {
                for argument in arguments.iter().skip(1) {
                    emit_argument_discard(self, argument)?;
                }

                let mut program = if let Ok(program) = frontend::parse_script_goal(argument_source)
                {
                    program
                } else {
                    self.emit_named_error_throw("SyntaxError")?;
                    return Ok(true);
                };

                if program
                    .functions
                    .iter()
                    .filter(|function| function.register_global)
                    .any(|function| is_non_definable_global_name(&function.name))
                {
                    self.emit_named_error_throw("TypeError")?;
                    return Ok(true);
                }

                self.with_isolated_indirect_eval_state(|compiler| {
                    let preexisting_locals =
                        compiler.locals.keys().cloned().collect::<HashSet<_>>();
                    let eval_local_function_declarations = if program.strict {
                        HashMap::new()
                    } else {
                        collect_eval_local_function_declarations(
                            &program.statements,
                            &program
                                .functions
                                .iter()
                                .filter(|function| is_eval_local_function_candidate(function))
                                .map(|function| function.name.clone())
                                .collect::<HashSet<_>>(),
                        )
                    };
                    compiler.prepare_eval_lexical_bindings(
                        &mut program.statements,
                        &eval_local_function_declarations,
                    )?;
                    compiler.register_bindings_skipping_eval_local_function_declarations(
                        &program.statements,
                        &eval_local_function_declarations,
                    )?;
                    if compiler.eval_program_declares_var_collision_with_global_lexical(&program) {
                        compiler.emit_named_error_throw("SyntaxError")?;
                        return Ok(());
                    }
                    if compiler.eval_program_declares_var_collision_with_active_lexical(&program) {
                        compiler.emit_named_error_throw("SyntaxError")?;
                        return Ok(());
                    }
                    if program.strict {
                        compiler.register_eval_global_function_local_bindings(&program.functions);
                    }
                    compiler.instantiate_eval_var_bindings(&program, &preexisting_locals)?;
                    if program.strict {
                        let strict_global_function_declarations = program
                            .functions
                            .iter()
                            .filter(|function| function.register_global)
                            .map(|function| (function.name.clone(), function.name.clone()))
                            .collect::<HashMap<_, _>>();
                        compiler.instantiate_eval_local_functions(
                            &strict_global_function_declarations,
                        )?;
                    } else {
                        compiler.instantiate_eval_global_functions(&program.functions)?;
                    }
                    compiler.instantiate_eval_local_functions(&eval_local_function_declarations)?;

                    let previous_strict_mode = compiler.strict_mode;
                    compiler.strict_mode = program.strict;
                    let emit_result = compiler.with_active_eval_lexical_scope(
                        collect_direct_eval_lexical_binding_names(&program.statements),
                        |compiler| {
                            let eval_statements = program
                                .statements
                                .iter()
                                .filter(|statement| {
                                    !is_eval_local_function_declaration_statement(
                                        statement,
                                        &eval_local_function_declarations,
                                    )
                                })
                                .collect::<Vec<_>>();

                            if let Some((last, rest)) = eval_statements.split_last() {
                                for statement in rest {
                                    compiler.emit_statement(statement)?;
                                }

                                match *last {
                                    Statement::Expression(expression) => {
                                        compiler.emit_numeric_expression(expression)?
                                    }
                                    Statement::Assign { name, value } => compiler
                                        .emit_numeric_expression(&Expression::Assign {
                                            name: name.clone(),
                                            value: Box::new(value.clone()),
                                        })?,
                                    Statement::AssignMember {
                                        object,
                                        property,
                                        value,
                                    } => compiler.emit_numeric_expression(
                                        &Expression::AssignMember {
                                            object: Box::new(object.clone()),
                                            property: Box::new(property.clone()),
                                            value: Box::new(value.clone()),
                                        },
                                    )?,
                                    _ => {
                                        compiler.emit_statement(last)?;
                                        compiler.push_i32_const(JS_UNDEFINED_TAG);
                                    }
                                }
                            } else {
                                compiler.push_i32_const(JS_UNDEFINED_TAG);
                            }

                            Ok(())
                        },
                    );
                    compiler.strict_mode = previous_strict_mode;
                    emit_result
                })?;

                Ok(true)
            }
            _ => {
                match argument {
                    CallArgument::Expression(expression) => {
                        self.emit_numeric_expression(expression)?
                    }
                    CallArgument::Spread(expression) => {
                        self.emit_numeric_expression(expression)?;
                        self.instructions.push(0x1a);
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                }

                for argument in arguments.iter().skip(1) {
                    emit_argument_discard(self, argument)?;
                }

                Ok(true)
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn instantiate_eval_global_functions(
        &mut self,
        functions: &[FunctionDeclaration],
    ) -> DirectResult<()> {
        for function in functions {
            if !function.register_global {
                continue;
            }
            let value_expression = Expression::Identifier(function.name.clone());
            self.module
                .global_kinds
                .insert(function.name.clone(), StaticValueKind::Function);
            self.module
                .global_value_bindings
                .insert(function.name.clone(), value_expression.clone());
            self.module.global_function_bindings.insert(
                function.name.clone(),
                LocalFunctionBinding::User(function.name.clone()),
            );
            self.instantiate_eval_global_function_property_descriptor(&function.name);
            let value_local = self.allocate_temp_local();
            let Some(user_function) = self.module.user_function_map.get(&function.name) else {
                return Err(Unsupported("eval global function runtime value"));
            };
            self.push_i32_const(user_function_runtime_value(user_function));
            self.push_local_set(value_local);
            self.emit_store_identifier_value_local(&function.name, &value_expression, value_local)?;
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn instantiate_eval_var_bindings(
        &mut self,
        program: &Program,
        preexisting_locals: &HashSet<String>,
    ) -> DirectResult<()> {
        let eval_var_names = collect_eval_var_names(program)
            .into_iter()
            .collect::<BTreeSet<_>>();
        for name in eval_var_names {
            if self.top_level_function && !program.strict {
                if !preexisting_locals.contains(&name) {
                    self.locals.remove(&name);
                    self.local_kinds.remove(&name);
                    self.local_value_bindings.remove(&name);
                    self.local_function_bindings.remove(&name);
                    self.local_specialized_function_values.remove(&name);
                    self.local_proxy_bindings.remove(&name);
                    self.local_array_bindings.remove(&name);
                    self.local_resizable_array_buffer_bindings.remove(&name);
                    self.local_typed_array_view_bindings.remove(&name);
                    self.runtime_typed_array_oob_locals.remove(&name);
                    self.tracked_array_function_values.remove(&name);
                    self.runtime_array_slots.remove(&name);
                    self.local_array_iterator_bindings.remove(&name);
                    self.local_iterator_step_bindings.remove(&name);
                    self.runtime_array_length_locals.remove(&name);
                    self.local_object_bindings.remove(&name);
                    self.local_prototype_object_bindings.remove(&name);
                    self.local_arguments_bindings.remove(&name);
                    self.local_descriptor_bindings.remove(&name);
                }
                if self.module.global_bindings.contains_key(&name) {
                    continue;
                }
                if let Some(binding) = self.module.implicit_global_bindings.get(&name).copied() {
                    self.ensure_global_property_descriptor_value(
                        &name,
                        &Expression::Undefined,
                        true,
                    );
                    self.push_global_get(binding.present_index);
                    self.instructions.push(0x45);
                    self.instructions.push(0x04);
                    self.instructions.push(EMPTY_BLOCK_TYPE);
                    self.push_control_frame();
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    self.push_global_set(binding.value_index);
                    self.push_i32_const(1);
                    self.push_global_set(binding.present_index);
                    self.instructions.push(0x0b);
                    self.pop_control_frame();
                    continue;
                }
                let binding = self.module.ensure_implicit_global_binding(&name);
                self.ensure_global_property_descriptor_value(&name, &Expression::Undefined, true);
                let value_local = self.allocate_temp_local();
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_local_set(value_local);
                self.emit_store_implicit_global_from_local(binding, value_local)?;
                continue;
            }

            if preexisting_locals.contains(&name) {
                continue;
            }
            let Some((resolved_name, local_index)) = self.resolve_current_local_binding(&name)
            else {
                continue;
            };
            self.local_value_bindings
                .insert(resolved_name.clone(), Expression::Undefined);
            self.local_kinds
                .insert(resolved_name, StaticValueKind::Undefined);
            self.push_i32_const(JS_UNDEFINED_TAG);
            self.push_local_set(local_index);
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn with_isolated_indirect_eval_state<T>(
        &mut self,
        callback: impl FnOnce(&mut Self) -> DirectResult<T>,
    ) -> DirectResult<T> {
        let saved_locals = std::mem::take(&mut self.locals);
        let saved_local_kinds = std::mem::take(&mut self.local_kinds);
        let saved_local_value_bindings = std::mem::take(&mut self.local_value_bindings);
        let saved_local_function_bindings = std::mem::take(&mut self.local_function_bindings);
        let saved_local_specialized_function_values =
            std::mem::take(&mut self.local_specialized_function_values);
        let saved_local_proxy_bindings = std::mem::take(&mut self.local_proxy_bindings);
        let saved_member_function_bindings = std::mem::take(&mut self.member_function_bindings);
        let saved_member_getter_bindings = std::mem::take(&mut self.member_getter_bindings);
        let saved_member_setter_bindings = std::mem::take(&mut self.member_setter_bindings);
        let saved_local_array_bindings = std::mem::take(&mut self.local_array_bindings);
        let saved_local_resizable_array_buffer_bindings =
            std::mem::take(&mut self.local_resizable_array_buffer_bindings);
        let saved_local_typed_array_view_bindings =
            std::mem::take(&mut self.local_typed_array_view_bindings);
        let saved_runtime_typed_array_oob_locals =
            std::mem::take(&mut self.runtime_typed_array_oob_locals);
        let saved_tracked_array_function_values =
            std::mem::take(&mut self.tracked_array_function_values);
        let saved_runtime_array_slots = std::mem::take(&mut self.runtime_array_slots);
        let saved_local_array_iterator_bindings =
            std::mem::take(&mut self.local_array_iterator_bindings);
        let saved_local_iterator_step_bindings =
            std::mem::take(&mut self.local_iterator_step_bindings);
        let saved_runtime_array_length_locals =
            std::mem::take(&mut self.runtime_array_length_locals);
        let saved_local_object_bindings = std::mem::take(&mut self.local_object_bindings);
        let saved_local_prototype_object_bindings =
            std::mem::take(&mut self.local_prototype_object_bindings);
        let saved_local_arguments_bindings = std::mem::take(&mut self.local_arguments_bindings);
        let saved_direct_arguments_aliases = std::mem::take(&mut self.direct_arguments_aliases);
        let saved_local_descriptor_bindings = std::mem::take(&mut self.local_descriptor_bindings);
        let saved_eval_lexical_initialized_locals =
            std::mem::take(&mut self.eval_lexical_initialized_locals);
        let saved_active_eval_lexical_scopes = std::mem::take(&mut self.active_eval_lexical_scopes);
        let saved_active_eval_lexical_binding_counts =
            std::mem::take(&mut self.active_eval_lexical_binding_counts);
        let saved_active_scoped_lexical_bindings =
            std::mem::take(&mut self.active_scoped_lexical_bindings);
        let saved_with_scopes = std::mem::take(&mut self.with_scopes);
        let saved_current_user_function_name = self.current_user_function_name.take();
        let saved_current_arguments_callee_present = self.current_arguments_callee_present;
        let saved_current_arguments_callee_override = self.current_arguments_callee_override.take();
        let saved_current_arguments_length_present = self.current_arguments_length_present;
        let saved_current_arguments_length_override = self.current_arguments_length_override.take();
        let saved_top_level_function = self.top_level_function;
        let saved_strict_mode = self.strict_mode;
        let saved_isolated_indirect_eval = self.isolated_indirect_eval;

        self.locals = HashMap::new();
        self.local_kinds = HashMap::new();
        self.local_value_bindings = HashMap::new();
        self.local_function_bindings = HashMap::new();
        self.local_specialized_function_values = HashMap::new();
        self.local_proxy_bindings = HashMap::new();
        self.member_function_bindings = HashMap::new();
        self.member_getter_bindings = HashMap::new();
        self.member_setter_bindings = HashMap::new();
        self.local_array_bindings = HashMap::new();
        self.local_resizable_array_buffer_bindings = HashMap::new();
        self.local_typed_array_view_bindings = HashMap::new();
        self.runtime_typed_array_oob_locals = HashMap::new();
        self.tracked_array_function_values = HashMap::new();
        self.runtime_array_slots = HashMap::new();
        self.local_array_iterator_bindings = HashMap::new();
        self.local_iterator_step_bindings = HashMap::new();
        self.runtime_array_length_locals = HashMap::new();
        self.local_object_bindings = HashMap::new();
        self.local_prototype_object_bindings = HashMap::new();
        self.local_arguments_bindings = HashMap::new();
        self.direct_arguments_aliases = HashSet::new();
        self.local_descriptor_bindings = HashMap::new();
        self.eval_lexical_initialized_locals = HashMap::new();
        self.active_eval_lexical_scopes = Vec::new();
        self.active_eval_lexical_binding_counts = HashMap::new();
        self.active_scoped_lexical_bindings = HashMap::new();
        self.with_scopes = Vec::new();
        self.current_arguments_callee_present = false;
        self.current_arguments_callee_override = None;
        self.current_arguments_length_present = false;
        self.current_arguments_length_override = None;
        self.top_level_function = true;
        self.strict_mode = false;
        self.isolated_indirect_eval = true;

        let result = callback(self);

        self.locals = saved_locals;
        self.local_kinds = saved_local_kinds;
        self.local_value_bindings = saved_local_value_bindings;
        self.local_function_bindings = saved_local_function_bindings;
        self.local_specialized_function_values = saved_local_specialized_function_values;
        self.local_proxy_bindings = saved_local_proxy_bindings;
        self.member_function_bindings = saved_member_function_bindings;
        self.member_getter_bindings = saved_member_getter_bindings;
        self.member_setter_bindings = saved_member_setter_bindings;
        self.local_array_bindings = saved_local_array_bindings;
        self.local_resizable_array_buffer_bindings = saved_local_resizable_array_buffer_bindings;
        self.local_typed_array_view_bindings = saved_local_typed_array_view_bindings;
        self.runtime_typed_array_oob_locals = saved_runtime_typed_array_oob_locals;
        self.tracked_array_function_values = saved_tracked_array_function_values;
        self.runtime_array_slots = saved_runtime_array_slots;
        self.local_array_iterator_bindings = saved_local_array_iterator_bindings;
        self.local_iterator_step_bindings = saved_local_iterator_step_bindings;
        self.runtime_array_length_locals = saved_runtime_array_length_locals;
        self.local_object_bindings = saved_local_object_bindings;
        self.local_prototype_object_bindings = saved_local_prototype_object_bindings;
        self.local_arguments_bindings = saved_local_arguments_bindings;
        self.direct_arguments_aliases = saved_direct_arguments_aliases;
        self.local_descriptor_bindings = saved_local_descriptor_bindings;
        self.eval_lexical_initialized_locals = saved_eval_lexical_initialized_locals;
        self.active_eval_lexical_scopes = saved_active_eval_lexical_scopes;
        self.active_eval_lexical_binding_counts = saved_active_eval_lexical_binding_counts;
        self.active_scoped_lexical_bindings = saved_active_scoped_lexical_bindings;
        self.with_scopes = saved_with_scopes;
        self.current_user_function_name = saved_current_user_function_name;
        self.current_arguments_callee_present = saved_current_arguments_callee_present;
        self.current_arguments_callee_override = saved_current_arguments_callee_override;
        self.current_arguments_length_present = saved_current_arguments_length_present;
        self.current_arguments_length_override = saved_current_arguments_length_override;
        self.top_level_function = saved_top_level_function;
        self.strict_mode = saved_strict_mode;
        self.isolated_indirect_eval = saved_isolated_indirect_eval;

        result
    }

    pub(in crate::backend::direct_wasm) fn register_eval_global_function_local_bindings(
        &mut self,
        functions: &[FunctionDeclaration],
    ) {
        for function in functions {
            if !function.register_global || self.locals.contains_key(&function.name) {
                continue;
            }
            self.locals
                .insert(function.name.clone(), self.next_local_index);
            self.local_kinds
                .insert(function.name.clone(), StaticValueKind::Unknown);
            self.next_local_index += 1;
        }
    }

    pub(in crate::backend::direct_wasm) fn instantiate_eval_local_functions(
        &mut self,
        declarations: &HashMap<String, String>,
    ) -> DirectResult<()> {
        for (binding_name, function_name) in declarations {
            let value_expression = Expression::Identifier(function_name.clone());
            let value_local = self.allocate_temp_local();
            self.emit_numeric_expression(&value_expression)?;
            self.push_local_set(value_local);
            if self.resolve_current_local_binding(binding_name).is_some()
                || self.module.global_bindings.contains_key(binding_name)
                || self
                    .resolve_eval_local_function_hidden_name(binding_name)
                    .is_some()
            {
                self.emit_store_identifier_value_local(
                    binding_name,
                    &value_expression,
                    value_local,
                )?;
            }
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn eval_arguments_declaration_conflicts(
        &self,
        program: &Program,
    ) -> bool {
        if !eval_program_declares_var_arguments(program) {
            return false;
        }

        let Some(current_function_name) = self.current_user_function_name.as_deref() else {
            return false;
        };
        let Some(current_function) = self.module.user_function_map.get(current_function_name)
        else {
            return false;
        };

        !current_function.lexical_this
            || current_function
                .params
                .iter()
                .any(|parameter| parameter == "arguments")
    }

    pub(in crate::backend::direct_wasm) fn eval_program_declares_var_collision_with_global_lexical(
        &self,
        program: &Program,
    ) -> bool {
        if !self.top_level_function || program.strict {
            return false;
        }

        collect_eval_var_names(program)
            .into_iter()
            .any(|name| self.module.global_lexical_bindings.contains(&name))
    }

    pub(in crate::backend::direct_wasm) fn eval_program_declares_var_collision_with_active_lexical(
        &self,
        program: &Program,
    ) -> bool {
        if program.strict {
            return false;
        }

        collect_eval_var_names(program)
            .into_iter()
            .any(|name| self.active_eval_lexical_binding_counts.contains_key(&name))
    }

    pub(in crate::backend::direct_wasm) fn eval_program_declares_non_definable_global_function(
        &self,
        program: &Program,
    ) -> bool {
        if !self.top_level_function {
            return false;
        }

        program
            .functions
            .iter()
            .filter(|function| function.register_global)
            .any(|function| is_non_definable_global_name(&function.name))
    }

    pub(in crate::backend::direct_wasm) fn parse_eval_program_in_current_function_context(
        &self,
        source: &str,
    ) -> Option<Program> {
        let current_function_name = self.current_user_function_name.as_deref()?;
        if self
            .resolve_home_object_name_for_function(current_function_name)
            .is_some()
            && source.contains("super")
        {
            if let Some(program) = self.parse_eval_program_in_method_context(source) {
                return Some(program);
            }
        }

        self.parse_eval_program_in_ordinary_function_context(source)
    }

    pub(in crate::backend::direct_wasm) fn parse_eval_program_in_ordinary_function_context(
        &self,
        source: &str,
    ) -> Option<Program> {
        let wrapper_name = "__ayy_eval_new_target_context__";
        let wrapped_source = format!("function {wrapper_name}() {{\n{source}\n}}");
        let mut wrapped_program = frontend::parse_script_goal(&wrapped_source).ok()?;
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

    pub(in crate::backend::direct_wasm) fn parse_eval_program_in_method_context(
        &self,
        source: &str,
    ) -> Option<Program> {
        let wrapper_property = "__ayy_eval_wrapper__";
        let wrapped_source = format!("({{{wrapper_property}() {{\n{source}\n}}}});");
        let mut wrapped_program = frontend::parse_script_goal(&wrapped_source).ok()?;
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

    pub(in crate::backend::direct_wasm) fn prepare_eval_lexical_bindings(
        &mut self,
        statements: &mut [Statement],
        eval_local_function_declarations: &HashMap<String, String>,
    ) -> DirectResult<()> {
        let lexical_names = statements
            .iter()
            .filter_map(|statement| match statement {
                Statement::Let { name, .. }
                    if !is_eval_local_function_declaration_statement(
                        statement,
                        eval_local_function_declarations,
                    ) =>
                {
                    Some(name.clone())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if lexical_names.is_empty() {
            return Ok(());
        }

        let mut renamed_bindings = HashMap::new();
        for name in lexical_names {
            if renamed_bindings.contains_key(&name) {
                continue;
            }
            let hidden_name =
                self.allocate_named_hidden_local("eval_lex", StaticValueKind::Unknown);
            let initialized_local = self.allocate_temp_local();
            let hidden_local = self
                .locals
                .get(&hidden_name)
                .copied()
                .expect("fresh hidden eval lexical local must exist");
            self.push_i32_const(JS_UNDEFINED_TAG);
            self.push_local_set(hidden_local);
            self.push_i32_const(0);
            self.push_local_set(initialized_local);
            self.eval_lexical_initialized_locals
                .insert(hidden_name.clone(), initialized_local);
            renamed_bindings.insert(name, hidden_name);
        }

        for statement in statements {
            self.rewrite_eval_lexical_statement(statement, &renamed_bindings);
        }

        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn prepare_eval_var_bindings(
        &mut self,
        statements: &mut [Statement],
        strict: bool,
    ) -> DirectResult<()> {
        if !strict {
            return Ok(());
        }

        let var_names = collect_eval_statement_var_names(statements)
            .into_iter()
            .collect::<Vec<_>>();
        if var_names.is_empty() {
            return Ok(());
        }

        let mut renamed_bindings = HashMap::new();
        for name in var_names {
            if renamed_bindings.contains_key(&name) {
                continue;
            }
            let hidden_name =
                self.allocate_named_hidden_local("eval_var", StaticValueKind::Undefined);
            let hidden_local = self
                .locals
                .get(&hidden_name)
                .copied()
                .expect("fresh hidden eval var local must exist");
            self.push_i32_const(JS_UNDEFINED_TAG);
            self.push_local_set(hidden_local);
            renamed_bindings.insert(name, hidden_name);
        }

        for statement in statements {
            self.rewrite_eval_lexical_statement(statement, &renamed_bindings);
        }

        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn register_bindings_skipping_eval_local_function_declarations(
        &mut self,
        statements: &[Statement],
        eval_local_function_declarations: &HashMap<String, String>,
    ) -> DirectResult<()> {
        for statement in statements {
            if is_eval_local_function_declaration_statement(
                statement,
                eval_local_function_declarations,
            ) {
                continue;
            }
            match statement {
                Statement::Block { body } | Statement::Labeled { body, .. } => self
                    .register_bindings_skipping_eval_local_function_declarations(
                        body,
                        eval_local_function_declarations,
                    )?,
                Statement::Var { name, .. } | Statement::Let { name, .. } => {
                    if self.top_level_function && self.module.global_bindings.contains_key(name) {
                        continue;
                    }
                    if self.locals.contains_key(name) {
                        continue;
                    }
                    self.locals.insert(name.clone(), self.next_local_index);
                    self.next_local_index += 1;
                }
                Statement::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    self.register_bindings_skipping_eval_local_function_declarations(
                        then_branch,
                        eval_local_function_declarations,
                    )?;
                    self.register_bindings_skipping_eval_local_function_declarations(
                        else_branch,
                        eval_local_function_declarations,
                    )?;
                }
                Statement::With { body, .. } => {
                    self.register_bindings_skipping_eval_local_function_declarations(
                        body,
                        eval_local_function_declarations,
                    )?;
                }
                Statement::While { body, .. } | Statement::DoWhile { body, .. } => self
                    .register_bindings_skipping_eval_local_function_declarations(
                        body,
                        eval_local_function_declarations,
                    )?,
                Statement::Try {
                    body,
                    catch_binding,
                    catch_setup,
                    catch_body,
                    ..
                } => {
                    self.register_bindings_skipping_eval_local_function_declarations(
                        body,
                        eval_local_function_declarations,
                    )?;
                    if let Some(catch_binding) = catch_binding {
                        if !self.locals.contains_key(catch_binding) {
                            self.locals
                                .insert(catch_binding.clone(), self.next_local_index);
                            self.local_kinds
                                .insert(catch_binding.clone(), StaticValueKind::Object);
                            self.next_local_index += 1;
                        }
                    }
                    self.register_bindings_skipping_eval_local_function_declarations(
                        catch_setup,
                        eval_local_function_declarations,
                    )?;
                    self.register_bindings_skipping_eval_local_function_declarations(
                        catch_body,
                        eval_local_function_declarations,
                    )?;
                }
                Statement::For {
                    init,
                    per_iteration_bindings,
                    body,
                    ..
                } => {
                    self.register_bindings_skipping_eval_local_function_declarations(
                        init,
                        eval_local_function_declarations,
                    )?;
                    for binding in per_iteration_bindings {
                        if self.locals.contains_key(binding) {
                            continue;
                        }
                        self.locals.insert(binding.clone(), self.next_local_index);
                        self.local_kinds
                            .insert(binding.clone(), StaticValueKind::Unknown);
                        self.next_local_index += 1;
                    }
                    self.register_bindings_skipping_eval_local_function_declarations(
                        body,
                        eval_local_function_declarations,
                    )?;
                }
                Statement::Switch {
                    bindings, cases, ..
                } => {
                    for binding in bindings {
                        if self.locals.contains_key(binding) {
                            continue;
                        }
                        self.locals.insert(binding.clone(), self.next_local_index);
                        self.local_kinds
                            .insert(binding.clone(), StaticValueKind::Unknown);
                        self.next_local_index += 1;
                    }
                    for case in cases {
                        self.register_bindings_skipping_eval_local_function_declarations(
                            &case.body,
                            eval_local_function_declarations,
                        )?;
                    }
                }
                Statement::Assign { .. }
                | Statement::Break { .. }
                | Statement::Continue { .. }
                | Statement::Expression(_)
                | Statement::Print { .. }
                | Statement::Return(_) => {}
                _ => {}
            }
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn rewrite_eval_lexical_statement(
        &self,
        statement: &mut Statement,
        renamed_bindings: &HashMap<String, String>,
    ) {
        match statement {
            Statement::Block { body } | Statement::Labeled { body, .. } => {
                for statement in body {
                    self.rewrite_eval_lexical_statement(statement, renamed_bindings);
                }
            }
            Statement::Var { name, value } => {
                if let Some(renamed_name) = renamed_bindings.get(name) {
                    *name = renamed_name.clone();
                }
                self.rewrite_eval_lexical_expression(value, renamed_bindings);
            }
            Statement::Let { name, value, .. } => {
                if let Some(renamed_name) = renamed_bindings.get(name) {
                    *name = renamed_name.clone();
                }
                self.rewrite_eval_lexical_expression(value, renamed_bindings);
            }
            Statement::Assign { name, value } => {
                if let Some(renamed_name) = renamed_bindings.get(name) {
                    *name = renamed_name.clone();
                }
                self.rewrite_eval_lexical_expression(value, renamed_bindings);
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                self.rewrite_eval_lexical_expression(object, renamed_bindings);
                self.rewrite_eval_lexical_expression(property, renamed_bindings);
                self.rewrite_eval_lexical_expression(value, renamed_bindings);
            }
            Statement::Print { values } => {
                for value in values {
                    self.rewrite_eval_lexical_expression(value, renamed_bindings);
                }
            }
            Statement::Expression(expression)
            | Statement::Throw(expression)
            | Statement::Return(expression)
            | Statement::Yield { value: expression }
            | Statement::YieldDelegate { value: expression } => {
                self.rewrite_eval_lexical_expression(expression, renamed_bindings);
            }
            Statement::With { object, body } => {
                self.rewrite_eval_lexical_expression(object, renamed_bindings);
                for statement in body {
                    self.rewrite_eval_lexical_statement(statement, renamed_bindings);
                }
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.rewrite_eval_lexical_expression(condition, renamed_bindings);
                for statement in then_branch {
                    self.rewrite_eval_lexical_statement(statement, renamed_bindings);
                }
                for statement in else_branch {
                    self.rewrite_eval_lexical_statement(statement, renamed_bindings);
                }
            }
            Statement::Try {
                body,
                catch_setup,
                catch_body,
                ..
            } => {
                for statement in body {
                    self.rewrite_eval_lexical_statement(statement, renamed_bindings);
                }
                for statement in catch_setup {
                    self.rewrite_eval_lexical_statement(statement, renamed_bindings);
                }
                for statement in catch_body {
                    self.rewrite_eval_lexical_statement(statement, renamed_bindings);
                }
            }
            Statement::Switch {
                discriminant,
                cases,
                ..
            } => {
                self.rewrite_eval_lexical_expression(discriminant, renamed_bindings);
                for case in cases {
                    if let Some(test) = &mut case.test {
                        self.rewrite_eval_lexical_expression(test, renamed_bindings);
                    }
                    for statement in &mut case.body {
                        self.rewrite_eval_lexical_statement(statement, renamed_bindings);
                    }
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
                for statement in init {
                    self.rewrite_eval_lexical_statement(statement, renamed_bindings);
                }
                if let Some(condition) = condition {
                    self.rewrite_eval_lexical_expression(condition, renamed_bindings);
                }
                if let Some(update) = update {
                    self.rewrite_eval_lexical_expression(update, renamed_bindings);
                }
                if let Some(break_hook) = break_hook {
                    self.rewrite_eval_lexical_expression(break_hook, renamed_bindings);
                }
                for statement in body {
                    self.rewrite_eval_lexical_statement(statement, renamed_bindings);
                }
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
                self.rewrite_eval_lexical_expression(condition, renamed_bindings);
                if let Some(break_hook) = break_hook {
                    self.rewrite_eval_lexical_expression(break_hook, renamed_bindings);
                }
                for statement in body {
                    self.rewrite_eval_lexical_statement(statement, renamed_bindings);
                }
            }
            Statement::Break { .. } | Statement::Continue { .. } => {}
        }
    }

    pub(in crate::backend::direct_wasm) fn rewrite_eval_lexical_expression(
        &self,
        expression: &mut Expression,
        renamed_bindings: &HashMap<String, String>,
    ) {
        match expression {
            Expression::Identifier(name) | Expression::Update { name, .. } => {
                if let Some(renamed_name) = renamed_bindings.get(name) {
                    *name = renamed_name.clone();
                }
            }
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        crate::ir::hir::ArrayElement::Expression(expression)
                        | crate::ir::hir::ArrayElement::Spread(expression) => {
                            self.rewrite_eval_lexical_expression(expression, renamed_bindings);
                        }
                    }
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        ObjectEntry::Data { key, value } => {
                            self.rewrite_eval_lexical_expression(key, renamed_bindings);
                            self.rewrite_eval_lexical_expression(value, renamed_bindings);
                        }
                        ObjectEntry::Getter { key, getter } => {
                            self.rewrite_eval_lexical_expression(key, renamed_bindings);
                            self.rewrite_eval_lexical_expression(getter, renamed_bindings);
                        }
                        ObjectEntry::Setter { key, setter } => {
                            self.rewrite_eval_lexical_expression(key, renamed_bindings);
                            self.rewrite_eval_lexical_expression(setter, renamed_bindings);
                        }
                        ObjectEntry::Spread(expression) => {
                            self.rewrite_eval_lexical_expression(expression, renamed_bindings);
                        }
                    }
                }
            }
            Expression::Member { object, property } => {
                self.rewrite_eval_lexical_expression(object, renamed_bindings);
                self.rewrite_eval_lexical_expression(property, renamed_bindings);
            }
            Expression::SuperMember { property } => {
                self.rewrite_eval_lexical_expression(property, renamed_bindings);
            }
            Expression::Assign { name, value } => {
                if let Some(renamed_name) = renamed_bindings.get(name) {
                    *name = renamed_name.clone();
                }
                self.rewrite_eval_lexical_expression(value, renamed_bindings);
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.rewrite_eval_lexical_expression(object, renamed_bindings);
                self.rewrite_eval_lexical_expression(property, renamed_bindings);
                self.rewrite_eval_lexical_expression(value, renamed_bindings);
            }
            Expression::AssignSuperMember { property, value } => {
                self.rewrite_eval_lexical_expression(property, renamed_bindings);
                self.rewrite_eval_lexical_expression(value, renamed_bindings);
            }
            Expression::Await(expression)
            | Expression::EnumerateKeys(expression)
            | Expression::GetIterator(expression)
            | Expression::IteratorClose(expression)
            | Expression::Unary { expression, .. } => {
                self.rewrite_eval_lexical_expression(expression, renamed_bindings);
            }
            Expression::Binary { left, right, .. } => {
                self.rewrite_eval_lexical_expression(left, renamed_bindings);
                self.rewrite_eval_lexical_expression(right, renamed_bindings);
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.rewrite_eval_lexical_expression(condition, renamed_bindings);
                self.rewrite_eval_lexical_expression(then_expression, renamed_bindings);
                self.rewrite_eval_lexical_expression(else_expression, renamed_bindings);
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.rewrite_eval_lexical_expression(expression, renamed_bindings);
                }
            }
            Expression::Call { callee, arguments }
            | Expression::SuperCall { callee, arguments }
            | Expression::New { callee, arguments } => {
                self.rewrite_eval_lexical_expression(callee, renamed_bindings);
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.rewrite_eval_lexical_expression(expression, renamed_bindings);
                        }
                    }
                }
            }
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::NewTarget
            | Expression::This
            | Expression::Sent => {}
        }
    }
}
