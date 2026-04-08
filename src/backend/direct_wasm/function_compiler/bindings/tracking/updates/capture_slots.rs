use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn update_capture_slot_binding_from_expression(
        &mut self,
        name: &str,
        value: &Expression,
    ) -> DirectResult<()> {
        self.update_local_function_binding(name, value);
        self.update_local_specialized_function_value(name, value)?;
        self.update_local_proxy_binding(name, value);
        self.update_local_array_binding(name, value);
        self.update_local_resizable_array_buffer_binding(name, value)?;
        self.update_local_typed_array_view_binding(name, value)?;
        self.update_local_array_iterator_binding(name, value);
        self.update_local_iterator_step_binding(name, value);
        self.update_local_object_binding(name, value);
        self.update_local_arguments_binding(name, value);
        self.update_local_descriptor_binding(name, value);
        self.update_local_value_binding(name, value);
        self.update_object_prototype_binding_from_value(name, value);
        let value_kind = self
            .infer_value_kind(value)
            .unwrap_or(StaticValueKind::Unknown);
        self.state
            .speculation
            .static_semantics
            .set_local_kind(name, value_kind);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn initialize_returned_member_capture_slots_for_bindings(
        &mut self,
        name: &str,
        value: &Expression,
        value_local: u32,
        bindings: &[ReturnedMemberFunctionBinding],
    ) -> DirectResult<HashMap<String, BTreeMap<String, String>>> {
        let Some((user_function, arguments)) = self.resolve_user_function_call_target(value) else {
            return Ok(HashMap::new());
        };
        if bindings.is_empty() {
            return Ok(HashMap::new());
        }
        let Some(function) = self
            .resolve_registered_function_declaration(&user_function.name)
            .cloned()
        else {
            return Ok(HashMap::new());
        };
        let call_snapshot_bindings = self
            .state
            .speculation
            .static_semantics
            .last_bound_user_function_call
            .as_ref()
            .filter(|snapshot| snapshot.function_name == user_function.name)
            .map(|snapshot| snapshot.updated_bindings.clone());
        let returned_identifier = collect_returned_identifier(&function.body);
        let local_aliases = collect_returned_member_local_aliases(&function.body);
        let mut initialized_slots: BTreeMap<String, String> = BTreeMap::new();
        let mut property_slots: HashMap<String, BTreeMap<String, String>> = HashMap::new();

        for binding in bindings {
            let LocalFunctionBinding::User(member_function_name) = &binding.binding else {
                continue;
            };
            let capture_bindings = if let Some(captures) = self
                .backend
                .function_registry
                .analysis
                .user_function_capture_bindings
                .get(member_function_name)
                .filter(|captures| !captures.is_empty())
                .cloned()
            {
                captures
            } else if let Some(returned_expression) = self
                .resolve_function_binding_static_return_expression_with_call_frame(
                    &binding.binding,
                    &[],
                    &returned_identifier
                        .as_ref()
                        .map(|name| Expression::Identifier(name.clone()))
                        .unwrap_or(Expression::Undefined),
                )
                && let Some(LocalFunctionBinding::User(returned_function_name)) =
                    self.resolve_function_binding_from_expression(&returned_expression)
                && let Some(captures) = self
                    .backend
                    .function_registry
                    .analysis
                    .user_function_capture_bindings
                    .get(&returned_function_name)
                    .filter(|captures| !captures.is_empty())
                    .cloned()
            {
                captures
            } else {
                continue;
            };
            let mut capture_slots = BTreeMap::new();
            for capture_name in capture_bindings.keys() {
                let slot_name = if let Some(existing) = initialized_slots.get(capture_name) {
                    existing.clone()
                } else {
                    let (source_expression, source_uses_value_local) = if returned_identifier
                        .as_ref()
                        .is_some_and(|returned_identifier| capture_name == returned_identifier)
                    {
                        (value.clone(), true)
                    } else if let Some(alias) = local_aliases.get(capture_name) {
                        (
                            self.substitute_user_function_argument_bindings(
                                alias,
                                &user_function,
                                &arguments,
                            ),
                            false,
                        )
                    } else if let Some(param_name) = user_function.params.iter().find(|param| {
                        *param == capture_name
                            || scoped_binding_source_name(param)
                                .is_some_and(|source_name| source_name == capture_name)
                    }) {
                        (
                            self.substitute_user_function_argument_bindings(
                                &Expression::Identifier(param_name.clone()),
                                &user_function,
                                &arguments,
                            ),
                            false,
                        )
                    } else if let Some(snapshot_value) =
                        call_snapshot_bindings.as_ref().and_then(|bindings| {
                            bindings.get(capture_name).or_else(|| {
                                scoped_binding_source_name(capture_name)
                                    .and_then(|source_name| bindings.get(source_name))
                            })
                        })
                    {
                        let source_binding_name = scoped_binding_source_name(capture_name)
                            .unwrap_or(capture_name)
                            .to_string();
                        (
                            if self
                                .user_function_capture_source_is_locally_bound(&source_binding_name)
                            {
                                Expression::Identifier(source_binding_name)
                            } else {
                                snapshot_value.clone()
                            },
                            false,
                        )
                    } else {
                        (Expression::Identifier(capture_name.clone()), false)
                    };
                    let hidden_name = self.allocate_named_hidden_local(
                        &format!("closure_slot_{}_{}", name, capture_name),
                        self.infer_value_kind(&source_expression)
                            .unwrap_or(StaticValueKind::Unknown),
                    );
                    let hidden_local = self
                        .state
                        .runtime
                        .locals
                        .get(&hidden_name)
                        .copied()
                        .expect("fresh closure capture slot local must exist");
                    if source_uses_value_local {
                        self.push_local_get(value_local);
                    } else {
                        self.emit_numeric_expression(&source_expression)?;
                    }
                    self.push_local_set(hidden_local);
                    self.update_capture_slot_binding_from_expression(
                        &hidden_name,
                        &source_expression,
                    )?;
                    if let Expression::Identifier(source_binding_name) = &source_expression {
                        self.state
                            .speculation
                            .static_semantics
                            .capture_slot_source_bindings
                            .insert(hidden_name.clone(), source_binding_name.clone());
                    }
                    initialized_slots.insert(capture_name.clone(), hidden_name.clone());
                    hidden_name
                };
                capture_slots.insert(capture_name.clone(), slot_name);
            }
            if !capture_slots.is_empty() {
                property_slots.insert(binding.property.clone(), capture_slots);
            }
        }

        Ok(property_slots)
    }

    pub(in crate::backend::direct_wasm) fn resolve_returned_member_capture_bindings_for_value(
        &self,
        value: &Expression,
    ) -> Option<HashMap<String, HashMap<String, Expression>>> {
        let (user_function, arguments) = self.resolve_user_function_call_target(value)?;
        if user_function.returned_member_function_bindings.is_empty() {
            return None;
        }
        let function = self
            .resolve_registered_function_declaration(&user_function.name)?
            .clone();
        let returned_identifier = collect_returned_identifier(&function.body)?;
        let local_aliases = collect_returned_member_local_aliases(&function.body);
        let mut property_bindings = HashMap::new();

        for binding in &user_function.returned_member_function_bindings {
            let LocalFunctionBinding::User(member_function_name) = &binding.binding else {
                continue;
            };
            let Some(captures) = self
                .backend
                .function_registry
                .analysis
                .user_function_capture_bindings
                .get(member_function_name)
            else {
                property_bindings.insert(binding.property.clone(), HashMap::new());
                continue;
            };
            let mut capture_bindings = HashMap::new();
            for capture_name in captures.keys() {
                let source_expression = if capture_name == &returned_identifier {
                    value.clone()
                } else if let Some(alias) = local_aliases.get(capture_name) {
                    self.substitute_user_function_argument_bindings(
                        alias,
                        &user_function,
                        &arguments,
                    )
                } else if let Some(param_name) = user_function.params.iter().find(|param| {
                    *param == capture_name
                        || scoped_binding_source_name(param)
                            .is_some_and(|source_name| source_name == capture_name)
                }) {
                    self.substitute_user_function_argument_bindings(
                        &Expression::Identifier(param_name.clone()),
                        &user_function,
                        &arguments,
                    )
                } else {
                    Expression::Identifier(capture_name.clone())
                };
                capture_bindings.insert(capture_name.clone(), source_expression);
            }
            property_bindings.insert(binding.property.clone(), capture_bindings);
        }

        Some(property_bindings)
    }
}
