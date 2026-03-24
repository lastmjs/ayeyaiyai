use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn tracked_direct_arguments_prefix_len(&self) -> u32 {
        let mut indices = self.arguments_slots.keys().copied().collect::<Vec<_>>();
        indices.sort_unstable();
        let mut next_index = 0;
        for index in indices {
            if index != next_index {
                break;
            }
            next_index += 1;
        }
        next_index
    }

    pub(in crate::backend::direct_wasm) fn resolve_iterator_source_kind(
        &self,
        expression: &Expression,
    ) -> Option<IteratorSourceKind> {
        if self.is_direct_arguments_object(expression) {
            return Some(IteratorSourceKind::DirectArguments {
                tracked_prefix_len: self.tracked_direct_arguments_prefix_len(),
            });
        }
        if let Expression::Identifier(name) = expression {
            if self.local_typed_array_view_bindings.contains_key(name) {
                return Some(IteratorSourceKind::TypedArrayView { name: name.clone() });
            }
        }
        if let Some((steps, completion_effects)) =
            self.resolve_array_prototype_simple_generator_source(expression)
        {
            return Some(IteratorSourceKind::SimpleGenerator {
                steps,
                completion_effects,
            });
        }
        if let Some(array_binding) = self.resolve_array_binding_from_expression(expression) {
            let length_local = match expression {
                Expression::Identifier(name)
                    if self.is_named_global_array_binding(name)
                        && (!self.top_level_function
                            || self.uses_global_runtime_array_state(name)) =>
                {
                    None
                }
                _ => self.runtime_array_length_local_for_expression(expression),
            };
            return Some(IteratorSourceKind::StaticArray {
                values: array_binding.values,
                keys_only: false,
                length_local,
                runtime_name: match expression {
                    Expression::Identifier(name)
                        if self
                            .runtime_array_length_local_for_expression(expression)
                            .is_some()
                            || (self.is_named_global_array_binding(name)
                                && (!self.top_level_function
                                    || self.uses_global_runtime_array_state(name))) =>
                    {
                        Some(name.clone())
                    }
                    _ => None,
                },
            });
        }
        if let Expression::Identifier(name) = expression
            && let Some(value) = self
                .local_value_bindings
                .get(name)
                .or_else(|| self.module.global_value_bindings.get(name))
            && !static_expression_matches(value, expression)
            && let Some(source) = self.resolve_iterator_source_kind(value)
        {
            return Some(source);
        }
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.resolve_iterator_source_kind(&materialized);
        }
        if let Expression::Call { callee, arguments } = expression {
            if arguments.is_empty() {
                if let Expression::Member { object, property } = callee.as_ref() {
                    if is_symbol_iterator_expression(property) {
                        return self.resolve_iterator_source_kind(object);
                    }
                    if matches!(property.as_ref(), Expression::String(name) if name == "keys") {
                        let array_binding = self.resolve_array_binding_from_expression(object)?;
                        let length_local = match object.as_ref() {
                            Expression::Identifier(name)
                                if self.is_named_global_array_binding(name)
                                    && (!self.top_level_function
                                        || self.uses_global_runtime_array_state(name)) =>
                            {
                                None
                            }
                            _ => self.runtime_array_length_local_for_expression(object),
                        };
                        return Some(IteratorSourceKind::StaticArray {
                            values: array_binding.values,
                            keys_only: true,
                            length_local,
                            runtime_name: match object.as_ref() {
                                Expression::Identifier(name)
                                    if self
                                        .runtime_array_length_local_for_expression(object)
                                        .is_some()
                                        || (self.is_named_global_array_binding(name)
                                            && (!self.top_level_function
                                                || self.uses_global_runtime_array_state(name))) =>
                                {
                                    Some(name.clone())
                                }
                                _ => None,
                            },
                        });
                    }
                }
            }
        }
        if let Some((steps, completion_effects)) = self.resolve_simple_generator_source(expression)
        {
            return Some(IteratorSourceKind::SimpleGenerator {
                steps,
                completion_effects,
            });
        }
        if let Some((_, returned_expression, _)) =
            self.analyze_effectful_iterator_source_call(expression)
        {
            return self.resolve_iterator_source_kind(&returned_expression);
        }
        let binding = self.resolve_static_iterable_binding_from_expression(expression)?;
        Some(IteratorSourceKind::StaticArray {
            values: binding.values,
            keys_only: false,
            length_local: None,
            runtime_name: None,
        })
    }

    pub(in crate::backend::direct_wasm) fn update_local_array_iterator_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let source_expression = match value {
            Expression::GetIterator(iterated) => iterated.as_ref(),
            Expression::Call { .. } if self.resolve_simple_generator_source(value).is_some() => {
                value
            }
            _ => {
                self.local_array_iterator_bindings.remove(name);
                return;
            }
        };
        let Some(source) = self.resolve_iterator_source_kind(source_expression) else {
            self.local_array_iterator_bindings.remove(name);
            return;
        };
        let index_local = self
            .resolve_local_array_iterator_binding_name(name)
            .and_then(|binding_name| self.local_array_iterator_bindings.get(&binding_name))
            .map(|binding| binding.index_local)
            .unwrap_or_else(|| self.allocate_temp_local());
        let static_index = match &source {
            IteratorSourceKind::StaticArray { length_local, .. }
                if length_local.is_none() || name.starts_with("__ayy_array_iter_") =>
            {
                Some(0)
            }
            IteratorSourceKind::SimpleGenerator { .. } => Some(0),
            _ => None,
        };
        self.local_array_iterator_bindings.insert(
            name.to_string(),
            ArrayIteratorBinding {
                source,
                index_local,
                static_index,
            },
        );
        self.push_i32_const(0);
        self.push_local_set(index_local);
        self.local_kinds
            .insert(name.to_string(), StaticValueKind::Object);
    }

    pub(in crate::backend::direct_wasm) fn update_local_iterator_step_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Expression::Call { callee, arguments } = value else {
            self.local_iterator_step_bindings.remove(name);
            return;
        };
        if !arguments.is_empty() {
            self.local_iterator_step_bindings.remove(name);
            return;
        }
        let Expression::Member { object, property } = callee.as_ref() else {
            self.local_iterator_step_bindings.remove(name);
            return;
        };
        if !matches!(property.as_ref(), Expression::String(property_name) if property_name == "next")
        {
            self.local_iterator_step_bindings.remove(name);
            return;
        }
        let Expression::Identifier(iterator_name) = object.as_ref() else {
            self.local_iterator_step_bindings.remove(name);
            return;
        };
        let iterator_binding_name = self
            .resolve_local_array_iterator_binding_name(iterator_name)
            .unwrap_or_else(|| iterator_name.clone());
        let Some(mut iterator_binding) = self
            .local_array_iterator_bindings
            .get(&iterator_binding_name)
            .cloned()
        else {
            self.local_iterator_step_bindings.remove(name);
            return;
        };
        let (done_local, value_local) = match self.local_iterator_step_bindings.get(name) {
            Some(IteratorStepBinding::Runtime {
                done_local,
                value_local,
                ..
            }) => (*done_local, *value_local),
            _ => (self.allocate_temp_local(), self.allocate_temp_local()),
        };
        let function_binding = match &iterator_binding.source {
            IteratorSourceKind::StaticArray {
                values, keys_only, ..
            } if !keys_only => {
                let bindings = values
                    .iter()
                    .flatten()
                    .map(|value| self.resolve_function_binding_from_expression(value))
                    .collect::<Option<Vec<_>>>();
                bindings.and_then(|bindings| {
                    if bindings.is_empty() {
                        None
                    } else if bindings
                        .iter()
                        .all(|binding| binding == bindings.first().expect("not empty"))
                    {
                        bindings.first().cloned()
                    } else if are_function_constructor_bindings(&bindings) {
                        Some(LocalFunctionBinding::Builtin(
                            FUNCTION_CONSTRUCTOR_FAMILY_BUILTIN.to_string(),
                        ))
                    } else {
                        None
                    }
                })
            }
            _ => None,
        };
        let current_static_index = iterator_binding.static_index;
        let (static_done, static_value) = match &iterator_binding.source {
            IteratorSourceKind::StaticArray {
                values,
                keys_only,
                length_local,
                runtime_name,
            } if length_local.is_none() && runtime_name.is_none() => {
                let static_done = current_static_index.map(|index| index >= values.len());
                let static_value = current_static_index.map(|index| {
                    if index >= values.len() {
                        Expression::Undefined
                    } else if *keys_only {
                        Expression::Number(index as f64)
                    } else {
                        values
                            .get(index)
                            .and_then(|value| value.clone())
                            .unwrap_or(Expression::Undefined)
                    }
                });
                (static_done, static_value)
            }
            IteratorSourceKind::SimpleGenerator { steps, .. } => match current_static_index {
                Some(index) if index < steps.len() => match &steps[index].outcome {
                    SimpleGeneratorStepOutcome::Yield(value) => (Some(false), Some(value.clone())),
                    SimpleGeneratorStepOutcome::Throw(_) => (None, None),
                },
                Some(_) => (Some(true), Some(Expression::Undefined)),
                None => (None, None),
            },
            _ => (None, None),
        };
        let current_index_local = self.allocate_temp_local();
        self.push_local_get(iterator_binding.index_local);
        self.push_local_set(current_index_local);

        match &iterator_binding.source {
            IteratorSourceKind::StaticArray {
                values,
                keys_only,
                length_local,
                runtime_name,
            } => {
                if let Some(current_index) = iterator_binding.static_index {
                    iterator_binding.static_index = Some(current_index.saturating_add(1));
                } else {
                    iterator_binding.static_index = None;
                }
                self.push_local_get(current_index_local);
                if let Some(length_local) = length_local {
                    self.push_local_get(*length_local);
                } else if let Some(runtime_name) = runtime_name {
                    if !self.emit_global_runtime_array_length_read(runtime_name) {
                        self.push_i32_const(values.len() as i32);
                    }
                } else {
                    self.push_i32_const(values.len() as i32);
                }
                self.push_binary_op(BinaryOp::GreaterThanOrEqual)
                    .expect("static iterator comparisons are supported");
                self.push_local_set(done_local);

                self.push_local_get(done_local);
                self.instructions.push(0x04);
                self.instructions.push(EMPTY_BLOCK_TYPE);
                self.push_control_frame();
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_local_set(value_local);
                self.instructions.push(0x05);
                if *keys_only {
                    self.push_local_get(current_index_local);
                } else if let Some(runtime_name) = runtime_name {
                    if !self
                        .emit_dynamic_runtime_array_slot_read_from_local(
                            runtime_name,
                            current_index_local,
                        )
                        .expect("dynamic runtime array iterator reads are supported")
                        && !self
                            .emit_dynamic_global_runtime_array_slot_read_from_local(
                                runtime_name,
                                current_index_local,
                            )
                            .expect("dynamic global runtime array iterator reads are supported")
                    {
                        self.emit_runtime_array_iterator_value_from_local(
                            current_index_local,
                            values,
                        )
                        .expect("static iterator values are supported");
                    }
                } else {
                    self.emit_runtime_array_iterator_value_from_local(current_index_local, &values)
                        .expect("static iterator values are supported");
                }
                self.push_local_set(value_local);
                self.push_local_get(current_index_local);
                self.push_i32_const(1);
                self.instructions.push(0x6a);
                self.push_local_set(iterator_binding.index_local);
                self.instructions.push(0x0b);
                self.pop_control_frame();
            }
            IteratorSourceKind::SimpleGenerator {
                steps,
                completion_effects,
            } => {
                iterator_binding.static_index = current_static_index.map(|index| {
                    if index >= steps.len() {
                        steps.len().saturating_add(1)
                    } else {
                        index.saturating_add(1)
                    }
                });
                let mut open_frames = 0;
                for (index, step) in steps.iter().enumerate() {
                    self.push_local_get(current_index_local);
                    self.push_i32_const(index as i32);
                    self.push_binary_op(BinaryOp::Equal)
                        .expect("generator iterator comparisons are supported");
                    self.instructions.push(0x04);
                    self.instructions.push(EMPTY_BLOCK_TYPE);
                    self.push_control_frame();
                    open_frames += 1;
                    if current_static_index == Some(index) {
                        for effect in &step.effects {
                            self.emit_statement(effect)
                                .expect("simple generator effects should be compilable");
                        }
                    } else {
                        self.with_restored_static_binding_metadata(|compiler| {
                            for effect in &step.effects {
                                compiler.emit_statement(effect)?;
                            }
                            Ok(())
                        })
                        .expect("simple generator effects should be compilable");
                    }
                    match &step.outcome {
                        SimpleGeneratorStepOutcome::Yield(value) => {
                            self.push_i32_const(0);
                            self.push_local_set(done_local);
                            self.emit_numeric_expression(value)
                                .expect("simple generator yields should be compilable");
                            self.push_local_set(value_local);
                            self.push_i32_const((index + 1) as i32);
                            self.push_local_set(iterator_binding.index_local);
                        }
                        SimpleGeneratorStepOutcome::Throw(value) => {
                            self.push_i32_const(1);
                            self.push_local_set(done_local);
                            self.push_i32_const(JS_UNDEFINED_TAG);
                            self.push_local_set(value_local);
                            self.push_i32_const((steps.len() + 1) as i32);
                            self.push_local_set(iterator_binding.index_local);
                            self.emit_statement(&Statement::Throw(value.clone()))
                                .expect("simple generator throw steps should be compilable");
                        }
                    }
                    self.instructions.push(0x05);
                }

                self.push_local_get(current_index_local);
                self.push_i32_const(steps.len() as i32);
                self.push_binary_op(BinaryOp::Equal)
                    .expect("generator completion comparisons are supported");
                self.instructions.push(0x04);
                self.instructions.push(EMPTY_BLOCK_TYPE);
                self.push_control_frame();
                if current_static_index == Some(steps.len()) {
                    for effect in completion_effects {
                        self.emit_statement(effect)
                            .expect("simple generator completion effects should be compilable");
                    }
                } else {
                    self.with_restored_static_binding_metadata(|compiler| {
                        for effect in completion_effects {
                            compiler.emit_statement(effect)?;
                        }
                        Ok(())
                    })
                    .expect("simple generator completion effects should be compilable");
                }
                self.push_i32_const(1);
                self.push_local_set(done_local);
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_local_set(value_local);
                self.push_i32_const((steps.len() + 1) as i32);
                self.push_local_set(iterator_binding.index_local);
                self.instructions.push(0x05);
                self.push_i32_const(1);
                self.push_local_set(done_local);
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_local_set(value_local);
                self.instructions.push(0x0b);
                self.pop_control_frame();

                for _ in 0..open_frames {
                    self.instructions.push(0x0b);
                    self.pop_control_frame();
                }
            }
            IteratorSourceKind::TypedArrayView { name: view_name } => {
                iterator_binding.static_index = None;
                let view_length_local = self
                    .runtime_array_length_locals
                    .get(view_name)
                    .copied()
                    .expect("typed array views should have runtime length locals");
                self.push_local_get(current_index_local);
                self.push_local_get(view_length_local);
                self.push_binary_op(BinaryOp::GreaterThanOrEqual)
                    .expect("typed array iterator comparisons are supported");
                self.push_local_set(done_local);

                self.push_local_get(done_local);
                self.instructions.push(0x04);
                self.instructions.push(EMPTY_BLOCK_TYPE);
                self.push_control_frame();
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_local_set(value_local);
                self.instructions.push(0x05);
                self.emit_dynamic_runtime_array_slot_read_from_local(
                    &view_name,
                    current_index_local,
                )
                .expect("typed array iterator reads are supported");
                self.push_local_set(value_local);
                self.push_local_get(current_index_local);
                self.push_i32_const(1);
                self.instructions.push(0x6a);
                self.push_local_set(iterator_binding.index_local);
                self.instructions.push(0x0b);
                self.pop_control_frame();
            }
            IteratorSourceKind::DirectArguments { tracked_prefix_len } => {
                iterator_binding.static_index = None;
                let effective_length_local = self.allocate_temp_local();
                if let Some(actual_argument_count_local) = self.actual_argument_count_local {
                    self.push_local_get(actual_argument_count_local);
                    self.push_i32_const(*tracked_prefix_len as i32);
                    self.push_binary_op(BinaryOp::LessThanOrEqual)
                        .expect("argument count comparisons are supported");
                    self.instructions.push(0x04);
                    self.instructions.push(EMPTY_BLOCK_TYPE);
                    self.push_control_frame();
                    self.push_local_get(actual_argument_count_local);
                    self.push_local_set(effective_length_local);
                    self.instructions.push(0x05);
                    self.push_i32_const(*tracked_prefix_len as i32);
                    self.push_local_set(effective_length_local);
                    self.instructions.push(0x0b);
                    self.pop_control_frame();
                } else {
                    self.push_i32_const(*tracked_prefix_len as i32);
                    self.push_local_set(effective_length_local);
                }

                self.push_local_get(current_index_local);
                self.push_local_get(effective_length_local);
                self.push_binary_op(BinaryOp::GreaterThanOrEqual)
                    .expect("argument iterator comparisons are supported");
                self.push_local_set(done_local);

                self.push_local_get(done_local);
                self.instructions.push(0x04);
                self.instructions.push(EMPTY_BLOCK_TYPE);
                self.push_control_frame();
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_local_set(value_local);
                self.instructions.push(0x05);
                self.emit_dynamic_direct_arguments_property_read_from_local(current_index_local)
                    .expect("direct arguments iteration reads are supported");
                self.push_local_set(value_local);
                self.push_local_get(current_index_local);
                self.push_i32_const(1);
                self.instructions.push(0x6a);
                self.push_local_set(iterator_binding.index_local);
                self.instructions.push(0x0b);
                self.pop_control_frame();
            }
        }

        self.local_array_iterator_bindings
            .insert(iterator_binding_name, iterator_binding);
        self.local_iterator_step_bindings.insert(
            name.to_string(),
            IteratorStepBinding::Runtime {
                done_local,
                value_local,
                function_binding,
                static_done,
                static_value,
            },
        );
        self.local_kinds
            .insert(name.to_string(), StaticValueKind::Object);
    }

    pub(in crate::backend::direct_wasm) fn resolve_iterator_step_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<IteratorStepBinding> {
        if let Expression::Identifier(name) = expression {
            if let Some(binding) = self.local_iterator_step_bindings.get(name) {
                return Some(binding.clone());
            }
            if let Some((resolved_name, _)) = self.resolve_current_local_binding(name)
                && let Some(binding) = self.local_iterator_step_bindings.get(&resolved_name)
            {
                return Some(binding.clone());
            }
        }
        let Expression::Identifier(name) = self.resolve_bound_alias_expression(expression)? else {
            return None;
        };
        self.local_iterator_step_bindings.get(&name).cloned()
    }

    pub(in crate::backend::direct_wasm) fn update_local_array_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Some(array_binding) = self.resolve_array_binding_from_expression(value) else {
            self.local_array_bindings.remove(name);
            self.runtime_array_slots.remove(name);
            self.tracked_array_function_values.remove(name);
            return;
        };
        let source_binding_name = if let Expression::Identifier(source_name) = value {
            self.resolve_runtime_array_binding_name(source_name)
        } else {
            None
        };
        let copy_internal_rest_runtime_state = source_binding_name
            .as_ref()
            .is_some_and(|source_name| source_name.starts_with("__ayy_array_rest_"));
        let length_local = if copy_internal_rest_runtime_state {
            self.ensure_runtime_array_length_local(name)
        } else if let Some(source_name) = source_binding_name.as_ref() {
            self.runtime_array_length_locals
                .get(source_name)
                .copied()
                .unwrap_or_else(|| self.ensure_runtime_array_length_local(name))
        } else {
            self.ensure_runtime_array_length_local(name)
        };
        self.runtime_array_length_locals
            .insert(name.to_string(), length_local);
        if copy_internal_rest_runtime_state {
            let source_name = source_binding_name
                .as_ref()
                .expect("rest runtime copy should have a source binding");
            if let Some(source_length_local) =
                self.runtime_array_length_locals.get(source_name).copied()
            {
                self.push_local_get(source_length_local);
            } else {
                self.push_i32_const(array_binding.values.len() as i32);
            }
        } else if let Some(source_length_local) =
            self.runtime_array_length_local_for_expression(value)
        {
            self.push_local_get(source_length_local);
        } else {
            self.push_i32_const(array_binding.values.len() as i32);
        }
        self.push_local_set(length_local);
        if copy_internal_rest_runtime_state {
            let source_name = source_binding_name
                .as_ref()
                .expect("rest runtime copy should have a source binding");
            for index in 0..TRACKED_ARRAY_SLOT_LIMIT {
                let target_slot = self.ensure_runtime_array_slot_entry(name, index);
                if let Some(source_slot) = self.runtime_array_slot(source_name, index) {
                    self.push_local_get(source_slot.value_local);
                    self.push_local_set(target_slot.value_local);
                    self.push_local_get(source_slot.present_local);
                    self.push_local_set(target_slot.present_local);
                } else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    self.push_local_set(target_slot.value_local);
                    self.push_i32_const(0);
                    self.push_local_set(target_slot.present_local);
                }
            }
        } else if let Some(source_name) = source_binding_name.as_ref() {
            if let Some(source_slots) = self.runtime_array_slots.get(source_name).cloned() {
                self.runtime_array_slots
                    .insert(name.to_string(), source_slots);
            } else {
                self.ensure_runtime_array_slots_for_binding(name, &array_binding);
            }
        } else {
            self.ensure_runtime_array_slots_for_binding(name, &array_binding);
        }
        self.local_array_bindings
            .insert(name.to_string(), array_binding);
        if let Some(source_name) = source_binding_name.as_ref() {
            if let Some(bindings) = self.tracked_array_function_values.get(source_name).cloned() {
                self.tracked_array_function_values
                    .insert(name.to_string(), bindings);
            } else {
                self.tracked_array_function_values.remove(name);
            }
        } else {
            self.tracked_array_function_values.remove(name);
        }
        self.local_kinds
            .insert(name.to_string(), StaticValueKind::Object);
    }
}
