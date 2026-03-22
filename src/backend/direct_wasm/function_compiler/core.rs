use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn intern_string(&mut self, bytes: Vec<u8>) -> (u32, u32) {
        if let Some(existing) = self.interned_strings.get(&bytes) {
            return *existing;
        }

        let offset = self.next_data_offset;
        let len = bytes.len() as u32;
        self.next_data_offset += len;
        self.string_data.push((offset, bytes.clone()));
        self.interned_strings.insert(bytes, (offset, len));
        (offset, len)
    }
}

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn new(
        module: &'a mut DirectWasmCompiler,
        user_function: Option<&UserFunction>,
        allow_return: bool,
        mapped_arguments: bool,
        strict_mode: bool,
        parameter_bindings: &HashMap<String, Option<LocalFunctionBinding>>,
        parameter_value_bindings: &HashMap<String, Option<Expression>>,
        parameter_array_bindings: &HashMap<String, Option<ArrayValueBinding>>,
        parameter_object_bindings: &HashMap<String, Option<ObjectValueBinding>>,
    ) -> DirectResult<Self> {
        let params = user_function
            .map(|function| function.params.as_slice())
            .unwrap_or(&[]);
        let parameter_defaults = user_function
            .map(|function| function.parameter_defaults.clone())
            .unwrap_or_default();
        let needs_parameter_scope_arguments_local = user_function.is_some_and(|function| {
            function.lexical_this
                && function.has_parameter_defaults()
                && function.body_declares_arguments_binding
        });
        let visible_param_count = params.len() as u32;
        let actual_argument_count_local = user_function
            .filter(|function| !function.lexical_this)
            .map(UserFunction::actual_argument_count_param);
        let mut extra_argument_param_locals = HashMap::new();
        let total_param_count = if let Some(user_function) = user_function {
            for index in &user_function.extra_argument_indices {
                if let Some(local_index) = user_function.extra_argument_param(*index) {
                    extra_argument_param_locals.insert(*index, local_index);
                }
            }
            user_function.wasm_param_count()
        } else {
            0
        };
        let mut locals = HashMap::new();
        let mut local_kinds = HashMap::new();
        let mut local_function_bindings = HashMap::new();
        let fallback_local_name = "__ayy_fallback_local";
        for (index, param) in params.iter().enumerate() {
            if !locals.contains_key(param) {
                locals.insert(param.clone(), index as u32);
            }
            local_kinds.insert(param.clone(), StaticValueKind::Unknown);
            if let Some(Some(binding)) = parameter_bindings.get(param) {
                local_function_bindings.insert(param.clone(), binding.clone());
            }
        }
        let mut local_value_bindings = HashMap::new();
        for param in params {
            if let Some(Some(binding)) = parameter_value_bindings.get(param) {
                local_value_bindings.insert(param.clone(), binding.clone());
            }
        }
        let mut local_array_bindings = HashMap::new();
        for param in params {
            if let Some(Some(binding)) = parameter_array_bindings.get(param) {
                local_array_bindings.insert(param.clone(), binding.clone());
            }
        }
        let mut local_object_bindings = HashMap::new();
        for param in params {
            if let Some(Some(binding)) = parameter_object_bindings.get(param) {
                local_object_bindings.insert(param.clone(), binding.clone());
            }
        }
        let fallback_index = total_param_count;
        locals.insert(fallback_local_name.to_string(), fallback_index);
        local_kinds.insert(fallback_local_name.to_string(), StaticValueKind::Unknown);
        let throw_tag_local = total_param_count + 1;
        let throw_value_local = total_param_count + 2;
        let mut next_local_index = total_param_count + 3;
        if let Some(user_function) = user_function {
            let mut scope_bindings = user_function
                .scope_bindings
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            scope_bindings.sort();
            for binding in scope_bindings {
                if binding == "arguments" || locals.contains_key(&binding) {
                    continue;
                }
                locals.insert(binding.clone(), next_local_index);
                local_kinds.insert(binding, StaticValueKind::Unknown);
                next_local_index += 1;
            }
        }
        let parameter_scope_arguments_local = if needs_parameter_scope_arguments_local {
            let local_index = next_local_index;
            next_local_index += 1;
            Some(local_index)
        } else {
            None
        };
        let mut parameter_initialized_locals = HashMap::new();
        if !parameter_defaults.is_empty() {
            for param in params {
                if parameter_initialized_locals.contains_key(param) {
                    continue;
                }
                parameter_initialized_locals.insert(param.clone(), next_local_index);
                next_local_index += 1;
            }
        }

        Ok(Self {
            module,
            parameter_names: params.to_vec(),
            parameter_defaults,
            parameter_initialized_locals,
            parameter_scope_arguments_local,
            in_parameter_default_initialization: false,
            locals,
            local_kinds,
            local_value_bindings,
            local_function_bindings,
            local_specialized_function_values: HashMap::new(),
            local_proxy_bindings: HashMap::new(),
            member_function_bindings: HashMap::new(),
            member_getter_bindings: HashMap::new(),
            member_setter_bindings: HashMap::new(),
            local_array_bindings,
            local_resizable_array_buffer_bindings: HashMap::new(),
            local_typed_array_view_bindings: HashMap::new(),
            runtime_typed_array_oob_locals: HashMap::new(),
            tracked_array_function_values: HashMap::new(),
            runtime_array_slots: HashMap::new(),
            local_array_iterator_bindings: HashMap::new(),
            local_iterator_step_bindings: HashMap::new(),
            runtime_array_length_locals: HashMap::new(),
            materializing_expression_keys: RefCell::new(HashSet::new()),
            local_object_bindings,
            local_prototype_object_bindings: HashMap::new(),
            local_arguments_bindings: HashMap::new(),
            direct_arguments_aliases: HashSet::new(),
            local_descriptor_bindings: HashMap::new(),
            eval_lexical_initialized_locals: HashMap::new(),
            throw_tag_local,
            throw_value_local,
            strict_mode,
            next_local_index,
            param_count: total_param_count,
            visible_param_count,
            actual_argument_count_local,
            extra_argument_param_locals,
            arguments_slots: HashMap::new(),
            mapped_arguments,
            current_user_function_name: user_function.map(|function| function.name.clone()),
            current_arguments_callee_present: user_function
                .is_some_and(|function| !function.lexical_this),
            current_arguments_callee_override: None,
            current_arguments_length_present: user_function
                .is_some_and(|function| !function.lexical_this),
            current_arguments_length_override: None,
            instructions: Vec::new(),
            control_stack: Vec::new(),
            loop_stack: Vec::new(),
            break_stack: Vec::new(),
            active_eval_lexical_scopes: Vec::new(),
            active_eval_lexical_binding_counts: HashMap::new(),
            active_scoped_lexical_bindings: HashMap::new(),
            with_scopes: Vec::new(),
            try_stack: Vec::new(),
            allow_return,
            top_level_function: user_function.is_none(),
            isolated_indirect_eval: false,
        })
    }

    pub(in crate::backend::direct_wasm) fn compile(
        mut self,
        statements: &[Statement],
    ) -> DirectResult<CompiledFunction> {
        self.register_bindings(statements)?;
        self.push_global_get(THROW_TAG_GLOBAL_INDEX);
        self.push_local_set(self.throw_tag_local);
        self.push_global_get(THROW_VALUE_GLOBAL_INDEX);
        self.push_local_set(self.throw_value_local);
        if let Some(parameter_scope_arguments_local) = self.parameter_scope_arguments_local {
            self.push_i32_const(JS_UNDEFINED_TAG);
            self.push_local_set(parameter_scope_arguments_local);
        }
        let parameter_initialized_locals = self
            .parameter_initialized_locals
            .values()
            .copied()
            .collect::<Vec<_>>();
        for initialized_local in parameter_initialized_locals {
            self.push_i32_const(0);
            self.push_local_set(initialized_local);
        }
        self.initialize_arguments_object(statements)?;
        self.initialize_parameter_defaults()?;
        self.emit_statements_in_direct_lexical_scope(statements)?;

        self.clear_local_throw_state();
        self.clear_global_throw_state();
        if self.allow_return {
            self.push_i32_const(JS_UNDEFINED_TAG);
        }

        Ok(CompiledFunction {
            local_count: self.next_local_index - self.param_count,
            instructions: self.instructions,
        })
    }

    pub(in crate::backend::direct_wasm) fn initialize_parameter_defaults(
        &mut self,
    ) -> DirectResult<()> {
        self.in_parameter_default_initialization = true;
        for (index, default) in self.parameter_defaults.clone().into_iter().enumerate() {
            let Some(parameter_name) = self.parameter_names.get(index).cloned() else {
                continue;
            };
            let parameter_local = index as u32;
            let initialized_local = self
                .parameter_initialized_locals
                .get(&parameter_name)
                .copied();

            let Some(default) = default else {
                if let Some(initialized_local) = initialized_local {
                    self.push_i32_const(1);
                    self.push_local_set(initialized_local);
                }
                continue;
            };

            self.push_local_get(parameter_local);
            self.push_i32_const(JS_UNDEFINED_TAG);
            self.push_binary_op(BinaryOp::Equal)?;
            self.instructions.push(0x04);
            self.instructions.push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();

            let default_value_local = self.allocate_temp_local();
            self.emit_numeric_expression(&default)?;
            self.push_local_set(default_value_local);
            self.emit_store_identifier_value_local(&parameter_name, &default, default_value_local)?;
            if let Some(initialized_local) = initialized_local {
                self.push_i32_const(1);
                self.push_local_set(initialized_local);
            }

            self.instructions.push(0x05);
            if let Some(initialized_local) = initialized_local {
                self.push_i32_const(1);
                self.push_local_set(initialized_local);
            }
            self.instructions.push(0x0b);
            self.pop_control_frame();
        }
        self.in_parameter_default_initialization = false;

        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn parameter_scope_arguments_local_for(
        &self,
        name: &str,
    ) -> Option<u32> {
        self.in_parameter_default_initialization
            .then_some(name)
            .filter(|name| *name == "arguments")
            .and(self.parameter_scope_arguments_local)
    }

    pub(in crate::backend::direct_wasm) fn emit_parameter_default_binding_read(
        &mut self,
        name: &str,
    ) -> DirectResult<bool> {
        if !self.in_parameter_default_initialization {
            return Ok(false);
        }
        let Some((resolved_name, local_index)) = self.resolve_current_local_binding(name) else {
            return Ok(false);
        };
        let Some(initialized_local) = self
            .parameter_initialized_locals
            .get(&resolved_name)
            .copied()
        else {
            return Ok(false);
        };
        self.push_local_get(initialized_local);
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_local_get(local_index);
        self.instructions.push(0x05);
        self.emit_named_error_throw("ReferenceError")?;
        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn allocate_named_hidden_local(
        &mut self,
        prefix: &str,
        kind: StaticValueKind,
    ) -> String {
        let name = format!("__ayy_{prefix}_{}", self.next_local_index);
        self.locals.insert(name.clone(), self.next_local_index);
        self.local_kinds.insert(name.clone(), kind);
        self.next_local_index += 1;
        name
    }
}
