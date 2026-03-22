use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_user_function_call(
        &mut self,
        user_function: &UserFunction,
        arguments: &[CallArgument],
    ) -> DirectResult<()> {
        self.emit_user_function_call_with_new_target_and_this(
            user_function,
            arguments,
            JS_UNDEFINED_TAG,
            if user_function.strict {
                JS_UNDEFINED_TAG
            } else {
                JS_TYPEOF_OBJECT_TAG
            },
        )
    }

    pub(in crate::backend::direct_wasm) fn emit_dynamic_user_function_call(
        &mut self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let callee_local = self.allocate_temp_local();
        self.emit_numeric_expression(callee)?;
        self.push_local_set(callee_local);

        self.push_local_get(callee_local);
        self.push_i32_const(JS_BUILTIN_EVAL_VALUE);
        self.push_binary_op(BinaryOp::Equal)?;
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.emit_indirect_eval_call(arguments)?;
        self.instructions.push(0x05);

        if self.module.user_functions.is_empty() {
            self.push_i32_const(JS_UNDEFINED_TAG);
            self.instructions.push(0x0b);
            self.pop_control_frame();
            return Ok(true);
        }

        let expanded_arguments = self.expand_call_arguments(arguments);
        let mut call_arguments = Vec::with_capacity(expanded_arguments.len());
        for (index, argument) in expanded_arguments.iter().enumerate() {
            let argument_value_local = self.allocate_temp_local();
            self.emit_numeric_expression(argument)?;
            self.push_local_set(argument_value_local);

            let hidden_name = self.allocate_named_hidden_local(
                &format!("dynamic_call_arg_{index}"),
                self.infer_value_kind(argument)
                    .unwrap_or(StaticValueKind::Unknown),
            );
            let hidden_local = self
                .locals
                .get(&hidden_name)
                .copied()
                .expect("fresh dynamic call hidden local must exist");
            self.push_local_get(argument_value_local);
            self.push_local_set(hidden_local);
            call_arguments.push(CallArgument::Expression(Expression::Identifier(
                hidden_name,
            )));
        }

        let user_functions = self.module.user_functions.clone();
        for (index, user_function) in user_functions.iter().enumerate() {
            self.push_local_get(callee_local);
            self.push_i32_const(user_function_runtime_value(user_function));
            self.push_binary_op(BinaryOp::Equal)?;
            self.instructions.push(0x04);
            self.instructions.push(I32_TYPE);
            self.push_control_frame();
            self.emit_user_function_call(user_function, &call_arguments)?;
            self.instructions.push(0x05);
            if index + 1 == user_functions.len() {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
        }
        for _ in 0..user_functions.len() {
            self.instructions.push(0x0b);
            self.pop_control_frame();
        }
        self.instructions.push(0x0b);
        self.pop_control_frame();

        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_user_function_call_with_new_target(
        &mut self,
        user_function: &UserFunction,
        arguments: &[CallArgument],
        new_target_value: i32,
    ) -> DirectResult<()> {
        self.emit_user_function_call_with_new_target_and_this(
            user_function,
            arguments,
            new_target_value,
            JS_TYPEOF_OBJECT_TAG,
        )
    }

    pub(in crate::backend::direct_wasm) fn emit_user_function_call_with_new_target_and_this(
        &mut self,
        user_function: &UserFunction,
        arguments: &[CallArgument],
        new_target_value: i32,
        this_value: i32,
    ) -> DirectResult<()> {
        let expanded_arguments = self.expand_call_arguments(arguments);
        if user_function.is_generator()
            && expanded_arguments.is_empty()
            && self
                .analyze_simple_generator_function(&user_function.name)
                .is_some()
        {
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
            return Ok(());
        }
        if self.can_inline_user_function_call(user_function, &expanded_arguments) {
            for argument in &expanded_arguments {
                self.emit_numeric_expression(argument)?;
                self.instructions.push(0x1a);
            }
            if self.emit_inline_user_function_summary_with_arguments(
                user_function,
                &expanded_arguments,
            )? {
                return Ok(());
            }
        }

        let prepared_capture_bindings =
            self.prepare_user_function_capture_bindings(user_function)?;

        self.emit_prepared_user_function_call_with_new_target_and_this(
            user_function,
            &expanded_arguments,
            new_target_value,
            this_value,
            prepared_capture_bindings,
        )
    }

    pub(in crate::backend::direct_wasm) fn emit_user_function_call_without_inline_with_new_target_and_this(
        &mut self,
        user_function: &UserFunction,
        arguments: &[CallArgument],
        new_target_value: i32,
        this_value: i32,
    ) -> DirectResult<()> {
        let expanded_arguments = self.expand_call_arguments(arguments);
        let prepared_capture_bindings =
            self.prepare_user_function_capture_bindings(user_function)?;
        self.emit_prepared_user_function_call_with_new_target_and_this(
            user_function,
            &expanded_arguments,
            new_target_value,
            this_value,
            prepared_capture_bindings,
        )
    }

    pub(in crate::backend::direct_wasm) fn emit_prepared_user_function_call_with_new_target_and_this(
        &mut self,
        user_function: &UserFunction,
        expanded_arguments: &[Expression],
        new_target_value: i32,
        this_value: i32,
        prepared_capture_bindings: Vec<PreparedCaptureBinding>,
    ) -> DirectResult<()> {
        let saved_new_target_local = if user_function.lexical_this {
            None
        } else {
            let saved_local = self.allocate_temp_local();
            self.push_global_get(CURRENT_NEW_TARGET_GLOBAL_INDEX);
            self.push_local_set(saved_local);
            self.push_i32_const(new_target_value);
            self.push_global_set(CURRENT_NEW_TARGET_GLOBAL_INDEX);
            Some(saved_local)
        };
        let saved_this_local = if user_function.lexical_this {
            None
        } else {
            let saved_local = self.allocate_temp_local();
            self.push_global_get(CURRENT_THIS_GLOBAL_INDEX);
            self.push_local_set(saved_local);
            self.push_i32_const(this_value);
            self.push_global_set(CURRENT_THIS_GLOBAL_INDEX);
            Some(saved_local)
        };

        self.emit_prepare_user_function_capture_globals(&user_function.name)?;

        let visible_param_count = user_function.visible_param_count() as usize;
        let tracked_extra_indices = user_function
            .extra_argument_indices
            .iter()
            .map(|index| *index as usize)
            .collect::<HashSet<_>>();
        let mut argument_locals = HashMap::new();

        for (argument_index, argument) in expanded_arguments.iter().enumerate() {
            if argument_index < visible_param_count
                || tracked_extra_indices.contains(&argument_index)
            {
                let argument_local = self.allocate_temp_local();
                self.emit_numeric_expression(argument)?;
                self.push_local_set(argument_local);
                argument_locals.insert(argument_index, argument_local);
            } else {
                self.emit_numeric_expression(argument)?;
                self.instructions.push(0x1a);
            }
        }

        for argument_index in 0..visible_param_count {
            if let Some(argument_local) = argument_locals.get(&argument_index).copied() {
                self.push_local_get(argument_local);
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
        }

        self.push_i32_const(expanded_arguments.len() as i32);

        for index in &user_function.extra_argument_indices {
            if let Some(argument_local) = argument_locals.get(&(*index as usize)).copied() {
                self.push_local_get(argument_local);
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
        }
        self.push_call(user_function.function_index);
        let return_value_local = self.allocate_temp_local();
        self.push_local_set(return_value_local);
        self.restore_user_function_capture_bindings(&prepared_capture_bindings);
        if let Some(saved_new_target_local) = saved_new_target_local {
            self.push_local_get(saved_new_target_local);
            self.push_global_set(CURRENT_NEW_TARGET_GLOBAL_INDEX);
        }
        if let Some(saved_this_local) = saved_this_local {
            self.push_local_get(saved_this_local);
            self.push_global_set(CURRENT_THIS_GLOBAL_INDEX);
        }
        if user_function.is_async() {
            self.push_global_get(THROW_TAG_GLOBAL_INDEX);
            self.push_i32_const(0);
            self.push_binary_op(BinaryOp::NotEqual)?;
            self.instructions.push(0x04);
            self.instructions.push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.clear_global_throw_state();
            self.instructions.push(0x0b);
            self.pop_control_frame();
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
            return Ok(());
        }
        self.emit_check_global_throw_for_user_call()?;
        self.push_local_get(return_value_local);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_user_function_call_with_new_target_and_this_expression(
        &mut self,
        user_function: &UserFunction,
        arguments: &[CallArgument],
        new_target_value: i32,
        this_expression: &Expression,
    ) -> DirectResult<()> {
        let expanded_arguments = self.expand_call_arguments(arguments);
        if self.can_inline_user_function_call(user_function, &expanded_arguments) {
            self.emit_numeric_expression(this_expression)?;
            self.instructions.push(0x1a);
            for argument in &expanded_arguments {
                self.emit_numeric_expression(argument)?;
                self.instructions.push(0x1a);
            }
            if self.emit_inline_user_function_summary_with_arguments(
                user_function,
                &expanded_arguments,
            )? {
                return Ok(());
            }
        }

        let prepared_capture_bindings =
            self.prepare_user_function_capture_bindings(user_function)?;

        let saved_new_target_local = if user_function.lexical_this {
            None
        } else {
            let saved_local = self.allocate_temp_local();
            self.push_global_get(CURRENT_NEW_TARGET_GLOBAL_INDEX);
            self.push_local_set(saved_local);
            self.push_i32_const(new_target_value);
            self.push_global_set(CURRENT_NEW_TARGET_GLOBAL_INDEX);
            Some(saved_local)
        };
        let saved_this_local = if user_function.lexical_this {
            None
        } else {
            let saved_local = self.allocate_temp_local();
            let this_local = self.allocate_temp_local();
            self.push_global_get(CURRENT_THIS_GLOBAL_INDEX);
            self.push_local_set(saved_local);
            self.emit_numeric_expression(this_expression)?;
            self.push_local_set(this_local);
            self.push_local_get(this_local);
            self.push_global_set(CURRENT_THIS_GLOBAL_INDEX);
            Some(saved_local)
        };

        self.emit_prepare_user_function_capture_globals(&user_function.name)?;

        let visible_param_count = user_function.visible_param_count() as usize;
        let tracked_extra_indices = user_function
            .extra_argument_indices
            .iter()
            .map(|index| *index as usize)
            .collect::<HashSet<_>>();
        let mut argument_locals = HashMap::new();

        for (argument_index, argument) in expanded_arguments.iter().enumerate() {
            if argument_index < visible_param_count
                || tracked_extra_indices.contains(&argument_index)
            {
                let argument_local = self.allocate_temp_local();
                self.emit_numeric_expression(argument)?;
                self.push_local_set(argument_local);
                argument_locals.insert(argument_index, argument_local);
            } else {
                self.emit_numeric_expression(argument)?;
                self.instructions.push(0x1a);
            }
        }

        for argument_index in 0..visible_param_count {
            if let Some(argument_local) = argument_locals.get(&argument_index).copied() {
                self.push_local_get(argument_local);
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
        }

        self.push_i32_const(expanded_arguments.len() as i32);

        for index in &user_function.extra_argument_indices {
            if let Some(argument_local) = argument_locals.get(&(*index as usize)).copied() {
                self.push_local_get(argument_local);
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
        }
        self.push_call(user_function.function_index);
        let return_value_local = self.allocate_temp_local();
        self.push_local_set(return_value_local);
        self.restore_user_function_capture_bindings(&prepared_capture_bindings);
        if let Some(saved_new_target_local) = saved_new_target_local {
            self.push_local_get(saved_new_target_local);
            self.push_global_set(CURRENT_NEW_TARGET_GLOBAL_INDEX);
        }
        if let Some(saved_this_local) = saved_this_local {
            self.push_local_get(saved_this_local);
            self.push_global_set(CURRENT_THIS_GLOBAL_INDEX);
        }
        if user_function.is_async() {
            self.push_global_get(THROW_TAG_GLOBAL_INDEX);
            self.push_i32_const(0);
            self.push_binary_op(BinaryOp::NotEqual)?;
            self.instructions.push(0x04);
            self.instructions.push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.clear_global_throw_state();
            self.instructions.push(0x0b);
            self.pop_control_frame();
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
            return Ok(());
        }
        self.emit_check_global_throw_for_user_call()?;
        self.push_local_get(return_value_local);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn expand_apply_call_arguments_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<Vec<CallArgument>> {
        let materialized = self.materialize_static_expression(expression);
        match &materialized {
            Expression::Undefined | Expression::Null => Some(Vec::new()),
            _ => {
                if let Some(array_binding) =
                    self.resolve_array_binding_from_expression(&materialized)
                {
                    return Some(
                        array_binding
                            .values
                            .into_iter()
                            .map(|value| {
                                CallArgument::Expression(value.unwrap_or(Expression::Undefined))
                            })
                            .collect(),
                    );
                }
                self.resolve_arguments_binding_from_expression(&materialized)
                    .map(|binding| {
                        binding
                            .values
                            .into_iter()
                            .map(CallArgument::Expression)
                            .collect()
                    })
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_function_prototype_call_or_apply(
        &mut self,
        object: &Expression,
        property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let Expression::String(property_name) = property else {
            return Ok(false);
        };
        if property_name != "call" && property_name != "apply" {
            return Ok(false);
        }
        if property_name == "call" && self.emit_has_own_property_call(object, arguments)? {
            return Ok(true);
        }

        let Some(function_binding) = self.resolve_function_binding_from_expression(object) else {
            return Ok(false);
        };
        let LocalFunctionBinding::User(function_name) = function_binding else {
            return Ok(false);
        };
        let Some(user_function) = self.module.user_function_map.get(&function_name).cloned() else {
            return Ok(false);
        };

        let expanded_arguments = self.expand_call_arguments(arguments);
        let raw_this_expression = expanded_arguments
            .first()
            .cloned()
            .unwrap_or(Expression::Undefined);
        let (call_arguments, apply_expression) = if property_name == "call" {
            (
                expanded_arguments
                    .iter()
                    .skip(1)
                    .cloned()
                    .map(CallArgument::Expression)
                    .collect::<Vec<_>>(),
                None,
            )
        } else {
            let apply_expression = expanded_arguments
                .get(1)
                .cloned()
                .unwrap_or(Expression::Undefined);
            let Some(call_arguments) =
                self.expand_apply_call_arguments_from_expression(&apply_expression)
            else {
                return Ok(false);
            };
            (call_arguments, Some(apply_expression))
        };

        self.emit_numeric_expression(object)?;
        self.instructions.push(0x1a);
        let this_hidden_name = self.allocate_named_hidden_local(
            "call_apply_this",
            self.infer_value_kind(&raw_this_expression)
                .unwrap_or(StaticValueKind::Unknown),
        );
        let this_hidden_local = self
            .locals
            .get(&this_hidden_name)
            .copied()
            .expect("fresh call/apply hidden local must exist");
        self.emit_numeric_expression(&raw_this_expression)?;
        self.push_local_set(this_hidden_local);
        if let Some(apply_expression) = &apply_expression {
            self.emit_numeric_expression(apply_expression)?;
            self.instructions.push(0x1a);
            for extra_argument in expanded_arguments.iter().skip(2) {
                self.emit_numeric_expression(extra_argument)?;
                self.instructions.push(0x1a);
            }
        }
        self.emit_user_function_call_with_new_target_and_this_expression(
            &user_function,
            &call_arguments,
            JS_UNDEFINED_TAG,
            &Expression::Identifier(this_hidden_name),
        )?;
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_has_own_property_call(
        &mut self,
        object: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let Expression::Member {
            object: _target_object,
            property: target_property,
        } = object
        else {
            return Ok(false);
        };
        if !matches!(target_property.as_ref(), Expression::String(name) if name == "hasOwnProperty")
        {
            return Ok(false);
        }
        let [
            CallArgument::Expression(receiver),
            CallArgument::Expression(argument_property),
            rest @ ..,
        ] = arguments
        else {
            return Ok(false);
        };

        let result = if let Some(array_binding) =
            self.resolve_array_binding_from_expression(receiver)
        {
            Some(
                matches!(argument_property, Expression::String(property_name) if property_name == "length")
                    || argument_index_from_expression(argument_property).is_some_and(|index| {
                        array_binding
                            .values
                            .get(index as usize)
                            .is_some_and(|value| value.is_some())
                    }),
            )
        } else if let Some(object_binding) = self.resolve_object_binding_from_expression(receiver) {
            Some(
                self.resolve_object_binding_property_value(&object_binding, argument_property)
                    .is_some(),
            )
        } else if self
            .resolve_user_function_from_expression(receiver)
            .is_some_and(UserFunction::is_arrow)
        {
            match argument_property {
                Expression::String(property_name)
                    if property_name == "caller" || property_name == "arguments" =>
                {
                    Some(false)
                }
                _ => None,
            }
        } else {
            None
        };
        let Some(has_property) = result else {
            return Ok(false);
        };

        self.emit_numeric_expression(object)?;
        self.instructions.push(0x1a);
        self.emit_numeric_expression(receiver)?;
        self.instructions.push(0x1a);
        self.emit_numeric_expression(argument_property)?;
        self.instructions.push(0x1a);
        for argument in rest {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.instructions.push(0x1a);
                }
            }
        }
        self.push_i32_const(if has_property { 1 } else { 0 });
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn prepare_user_function_capture_bindings(
        &mut self,
        user_function: &UserFunction,
    ) -> DirectResult<Vec<PreparedCaptureBinding>> {
        let Some(capture_bindings) = self
            .module
            .user_function_capture_bindings
            .get(&user_function.name)
            .cloned()
        else {
            return Ok(Vec::new());
        };

        let mut prepared = Vec::new();
        for (_source_name, hidden_name) in capture_bindings {
            let binding = self
                .module
                .implicit_global_bindings
                .get(&hidden_name)
                .copied()
                .unwrap_or_else(|| self.module.ensure_implicit_global_binding(&hidden_name));
            let saved_value_local = self.allocate_temp_local();
            let saved_present_local = self.allocate_temp_local();
            self.push_global_get(binding.value_index);
            self.push_local_set(saved_value_local);
            self.push_global_get(binding.present_index);
            self.push_local_set(saved_present_local);
            prepared.push(PreparedCaptureBinding {
                binding,
                saved_value_local,
                saved_present_local,
            });
        }

        Ok(prepared)
    }

    pub(in crate::backend::direct_wasm) fn emit_prepare_user_function_capture_globals(
        &mut self,
        function_name: &str,
    ) -> DirectResult<()> {
        let Some(capture_bindings) = self
            .module
            .user_function_capture_bindings
            .get(function_name)
            .cloned()
        else {
            return Ok(());
        };

        for (source_name, hidden_name) in capture_bindings {
            let binding = self
                .module
                .implicit_global_bindings
                .get(&hidden_name)
                .copied()
                .unwrap_or_else(|| self.module.ensure_implicit_global_binding(&hidden_name));
            if !self.user_function_capture_source_is_locally_bound(&source_name) {
                self.clear_user_function_capture_static_metadata(&hidden_name);
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_global_set(binding.value_index);
                self.push_i32_const(0);
                self.push_global_set(binding.present_index);
                continue;
            }
            self.sync_user_function_capture_static_metadata(&source_name, &hidden_name);
            let value_local = self.allocate_temp_local();
            self.emit_numeric_expression(&Expression::Identifier(source_name))?;
            self.push_local_set(value_local);
            self.push_local_get(value_local);
            self.push_global_set(binding.value_index);
            self.push_i32_const(1);
            self.push_global_set(binding.present_index);
        }

        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn user_function_capture_source_is_locally_bound(
        &self,
        name: &str,
    ) -> bool {
        self.parameter_scope_arguments_local_for(name).is_some()
            || (name == "arguments" && self.has_arguments_object())
            || self.resolve_current_local_binding(name).is_some()
            || self.local_function_bindings.contains_key(name)
            || self.resolve_eval_local_function_hidden_name(name).is_some()
            || self
                .resolve_user_function_capture_hidden_name(name)
                .is_some()
    }

    pub(in crate::backend::direct_wasm) fn clear_user_function_capture_static_metadata(
        &mut self,
        hidden_name: &str,
    ) {
        self.module.global_value_bindings.remove(hidden_name);
        self.module.global_array_bindings.remove(hidden_name);
        self.module.global_object_bindings.remove(hidden_name);
        self.module.global_function_bindings.remove(hidden_name);
        self.module.global_kinds.remove(hidden_name);
    }

    pub(in crate::backend::direct_wasm) fn sync_user_function_capture_static_metadata(
        &mut self,
        source_name: &str,
        hidden_name: &str,
    ) {
        let source_expression = Expression::Identifier(source_name.to_string());
        let inferred_kind = self.infer_value_kind(&source_expression);
        let resolved_value = self.resolve_bound_alias_expression(&source_expression);

        if let Some(value) =
            resolved_value.filter(|value| !static_expression_matches(value, &source_expression))
        {
            self.module
                .global_value_bindings
                .insert(hidden_name.to_string(), value);
        } else {
            self.module.global_value_bindings.remove(hidden_name);
        }

        if let Some(array_binding) = self.resolve_array_binding_from_expression(&source_expression)
        {
            self.module
                .global_array_bindings
                .insert(hidden_name.to_string(), array_binding);
        } else {
            self.module.global_array_bindings.remove(hidden_name);
        }

        if let Some(object_binding) =
            self.resolve_object_binding_from_expression(&source_expression)
        {
            self.module
                .global_object_bindings
                .insert(hidden_name.to_string(), object_binding);
        } else {
            self.module.global_object_bindings.remove(hidden_name);
        }

        if let Some(function_binding) =
            self.resolve_function_binding_from_expression(&source_expression)
        {
            self.module
                .global_function_bindings
                .insert(hidden_name.to_string(), function_binding);
        } else {
            self.module.global_function_bindings.remove(hidden_name);
        }

        if let Some(kind) = inferred_kind {
            self.module
                .global_kinds
                .insert(hidden_name.to_string(), kind);
        } else {
            self.module.global_kinds.remove(hidden_name);
        }
    }

    pub(in crate::backend::direct_wasm) fn restore_user_function_capture_bindings(
        &mut self,
        prepared: &[PreparedCaptureBinding],
    ) {
        for binding in prepared.iter().rev() {
            self.push_local_get(binding.saved_value_local);
            self.push_global_set(binding.binding.value_index);
            self.push_local_get(binding.saved_present_local);
            self.push_global_set(binding.binding.present_index);
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_user_function_construct(
        &mut self,
        user_function: &UserFunction,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if !user_function.is_constructible() {
            return Ok(false);
        }

        self.emit_user_function_call_with_new_target(
            user_function,
            arguments,
            user_function_runtime_value(user_function),
        )?;
        self.instructions.push(0x1a);
        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn resolve_function_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<LocalFunctionBinding> {
        self.resolve_function_binding_from_expression_with_context(
            expression,
            self.current_user_function_name.as_deref(),
        )
    }

    pub(in crate::backend::direct_wasm) fn resolve_user_function_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<&UserFunction> {
        let LocalFunctionBinding::User(function_name) =
            self.resolve_function_binding_from_expression(expression)?
        else {
            return None;
        };
        self.module.user_function_map.get(&function_name)
    }

    pub(in crate::backend::direct_wasm) fn is_restricted_arrow_function_property(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> bool {
        matches!(
            property,
            Expression::String(property_name)
                if property_name == "caller" || property_name == "arguments"
        ) && self
            .resolve_user_function_from_expression(object)
            .is_some_and(UserFunction::is_arrow)
    }

    pub(in crate::backend::direct_wasm) fn resolve_function_binding_from_expression_with_context(
        &self,
        expression: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<LocalFunctionBinding> {
        if let Some(resolved) = self
            .resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
        {
            if let Some(binding) = self.resolve_function_binding_from_expression_with_context(
                &resolved,
                current_function_name,
            ) {
                return Some(binding);
            }
        }
        let binding = match expression {
            Expression::Identifier(name) => {
                if let Some((resolved_name, _)) = self.resolve_current_local_binding(name) {
                    self.local_function_bindings.get(&resolved_name).cloned()
                } else if self.resolve_eval_local_function_hidden_name(name).is_some() {
                    self.local_function_bindings.get(name).cloned()
                } else if builtin_function_runtime_value(name).is_some() {
                    Some(LocalFunctionBinding::Builtin(name.clone()))
                } else if let Some(function_binding) =
                    self.module.global_function_bindings.get(name)
                {
                    Some(function_binding.clone())
                } else if is_internal_user_function_identifier(name)
                    && self.module.user_function_map.contains_key(name)
                {
                    Some(LocalFunctionBinding::User(name.clone()))
                } else if name == "eval" || self.infer_call_result_kind(name).is_some() {
                    Some(LocalFunctionBinding::Builtin(name.clone()))
                } else {
                    None
                }
            }
            Expression::Sequence(expressions) => expressions.last().and_then(|expression| {
                self.resolve_function_binding_from_expression_with_context(
                    expression,
                    current_function_name,
                )
            }),
            Expression::Member { object, property } => {
                if matches!(property.as_ref(), Expression::String(name) if name == "constructor")
                    && self
                        .resolve_function_binding_from_expression(object)
                        .is_some()
                {
                    return Some(LocalFunctionBinding::Builtin(
                        FUNCTION_CONSTRUCTOR_FAMILY_BUILTIN.to_string(),
                    ));
                }
                if matches!(property.as_ref(), Expression::String(name) if name == "value") {
                    if let Some(IteratorStepBinding::Runtime {
                        function_binding: Some(function_binding),
                        ..
                    }) = self.resolve_iterator_step_binding_from_expression(object)
                    {
                        return Some(function_binding);
                    }
                }
                if let Some(value) =
                    self.resolve_returned_member_value_from_expression(object, property)
                {
                    self.resolve_function_binding_from_expression(&value)
                } else {
                    self.resolve_member_function_binding(object, property)
                }
            }
            Expression::SuperMember { property } => {
                self.resolve_super_function_binding_with_context(property, current_function_name)
            }
            _ => None,
        };
        if binding.is_some() {
            return binding;
        }

        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.resolve_function_binding_from_expression_with_context(
                &materialized,
                current_function_name,
            );
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_test262_realm_id_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<u32> {
        match expression {
            Expression::Identifier(name) => {
                if let Some(realm_id) = parse_test262_realm_identifier(name) {
                    return Some(realm_id);
                }
                let resolved = self.resolve_bound_alias_expression(expression)?;
                let Expression::Identifier(name) = resolved else {
                    return None;
                };
                parse_test262_realm_identifier(&name)
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_test262_realm_global_id_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<u32> {
        let materialized = self.materialize_static_expression(expression);
        match &materialized {
            Expression::Identifier(name) => parse_test262_realm_global_identifier(name),
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "global") => {
                self.resolve_test262_realm_id_from_expression(object)
            }
            _ => None,
        }
    }
}
