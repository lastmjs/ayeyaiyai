use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn update_member_function_binding_from_expression(
        &mut self,
        expression: &Expression,
    ) {
        let Expression::Call { callee, arguments } = expression else {
            return;
        };
        let Expression::Member { object, property } = callee.as_ref() else {
            return;
        };
        if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Object") {
            return;
        }
        if !matches!(property.as_ref(), Expression::String(name) if name == "defineProperty") {
            return;
        }

        let [
            CallArgument::Expression(target),
            CallArgument::Expression(property),
            CallArgument::Expression(descriptor),
            ..,
        ] = arguments.as_slice()
        else {
            return;
        };

        let Some(key) = self.member_function_binding_key(target, property) else {
            return;
        };
        let value_binding = self.resolve_function_binding_from_descriptor_expression(descriptor);
        let getter_binding = self.resolve_getter_binding_from_descriptor_expression(descriptor);
        let setter_binding = self.resolve_setter_binding_from_descriptor_expression(descriptor);

        if let Some(binding) = value_binding {
            self.member_function_bindings
                .insert(key.clone(), binding.clone());
            if self.binding_key_is_global(&key) {
                self.module
                    .global_member_function_bindings
                    .insert(key.clone(), binding);
            }
        } else {
            self.member_function_bindings.remove(&key);
            if self.binding_key_is_global(&key) {
                self.module.global_member_function_bindings.remove(&key);
            }
        }

        if let Some(binding) = getter_binding {
            self.member_getter_bindings
                .insert(key.clone(), binding.clone());
            if self.binding_key_is_global(&key) {
                self.module
                    .global_member_getter_bindings
                    .insert(key.clone(), binding);
            }
        } else {
            self.member_getter_bindings.remove(&key);
            if self.binding_key_is_global(&key) {
                self.module.global_member_getter_bindings.remove(&key);
            }
        }

        if let Some(binding) = setter_binding {
            self.member_setter_bindings
                .insert(key.clone(), binding.clone());
            if self.binding_key_is_global(&key) {
                self.module
                    .global_member_setter_bindings
                    .insert(key.clone(), binding);
            }
        } else {
            self.member_setter_bindings.remove(&key);
            if self.binding_key_is_global(&key) {
                self.module.global_member_setter_bindings.remove(&key);
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn update_member_function_assignment_binding(
        &mut self,
        object: &Expression,
        property: &Expression,
        value: &Expression,
    ) {
        let Some(key) = self.member_function_binding_key(object, property) else {
            return;
        };
        let value_binding = self.resolve_function_binding_from_expression(value);

        if let Some(binding) = value_binding {
            self.member_function_bindings
                .insert(key.clone(), binding.clone());
            if self.binding_key_is_global(&key) {
                self.module
                    .global_member_function_bindings
                    .insert(key.clone(), binding);
            }
        } else {
            self.member_function_bindings.remove(&key);
            if self.binding_key_is_global(&key) {
                self.module.global_member_function_bindings.remove(&key);
            }
        }

        self.member_getter_bindings.remove(&key);
        self.member_setter_bindings.remove(&key);
        if self.binding_key_is_global(&key) {
            self.module.global_member_getter_bindings.remove(&key);
            self.module.global_member_setter_bindings.remove(&key);
        }
    }

    pub(in crate::backend::direct_wasm) fn update_local_function_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Some(function_name) = self.resolve_function_binding_from_expression(value) else {
            self.local_function_bindings.remove(name);
            return;
        };
        self.local_function_bindings
            .insert(name.to_string(), function_name);
    }

    pub(in crate::backend::direct_wasm) fn clear_member_function_bindings_for_name(
        &mut self,
        name: &str,
    ) {
        self.member_function_bindings.retain(|key, _| {
            !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) | MemberFunctionBindingTarget::Prototype(target) if target == name)
        });
        self.member_function_capture_slots.retain(|key, _| {
            !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) | MemberFunctionBindingTarget::Prototype(target) if target == name)
        });
        self.member_getter_bindings.retain(|key, _| {
            !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) | MemberFunctionBindingTarget::Prototype(target) if target == name)
        });
        self.member_setter_bindings.retain(|key, _| {
            !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) | MemberFunctionBindingTarget::Prototype(target) if target == name)
        });
        if self.binding_name_is_global(name) {
            self.module.global_member_function_bindings.retain(|key, _| {
                !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) | MemberFunctionBindingTarget::Prototype(target) if target == name)
            });
            self.module.global_member_function_capture_slots.retain(|key, _| {
                !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) | MemberFunctionBindingTarget::Prototype(target) if target == name)
            });
            self.module.global_member_getter_bindings.retain(|key, _| {
                !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) | MemberFunctionBindingTarget::Prototype(target) if target == name)
            });
            self.module.global_member_setter_bindings.retain(|key, _| {
                !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) | MemberFunctionBindingTarget::Prototype(target) if target == name)
            });
        }
    }

    pub(in crate::backend::direct_wasm) fn clear_object_literal_member_bindings_for_name(
        &mut self,
        name: &str,
    ) {
        self.member_function_bindings.retain(|key, _| {
            !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) if target == name)
        });
        self.member_function_capture_slots.retain(|key, _| {
            !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) if target == name)
        });
        self.member_getter_bindings.retain(|key, _| {
            !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) if target == name)
        });
        self.member_setter_bindings.retain(|key, _| {
            !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) if target == name)
        });
        if self.binding_name_is_global(name) {
            self.module.global_member_function_bindings.retain(|key, _| {
                !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) if target == name)
            });
            self.module.global_member_function_capture_slots.retain(|key, _| {
                !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) if target == name)
            });
            self.module.global_member_getter_bindings.retain(|key, _| {
                !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) if target == name)
            });
            self.module.global_member_setter_bindings.retain(|key, _| {
                !matches!(&key.target, MemberFunctionBindingTarget::Identifier(target) if target == name)
            });
        }
    }

    pub(in crate::backend::direct_wasm) fn object_literal_member_function_bindings(
        &self,
        entries: &[crate::ir::hir::ObjectEntry],
    ) -> Vec<ReturnedMemberFunctionBinding> {
        entries
            .iter()
            .filter_map(|entry| {
                let crate::ir::hir::ObjectEntry::Data { key, value } = entry else {
                    return None;
                };
                let Expression::String(property) = key else {
                    return None;
                };
                let binding = self.resolve_function_binding_from_expression(value)?;
                Some(ReturnedMemberFunctionBinding {
                    target: ReturnedMemberFunctionBindingTarget::Value,
                    property: property.clone(),
                    binding,
                })
            })
            .collect()
    }

    pub(in crate::backend::direct_wasm) fn inherited_member_function_bindings(
        &self,
        value: &Expression,
    ) -> Vec<ReturnedMemberFunctionBinding> {
        match value {
            Expression::Identifier(source_name) => self
                .member_function_bindings
                .iter()
                .chain(self.module.global_member_function_bindings.iter())
                .filter_map(|(key, binding)| match &key.target {
                    MemberFunctionBindingTarget::Identifier(target) if target == source_name => {
                        let MemberFunctionBindingProperty::String(property) = &key.property else {
                            return None;
                        };
                        Some(ReturnedMemberFunctionBinding {
                            target: ReturnedMemberFunctionBindingTarget::Value,
                            property: property.clone(),
                            binding: binding.clone(),
                        })
                    }
                    MemberFunctionBindingTarget::Prototype(target) if target == source_name => {
                        let MemberFunctionBindingProperty::String(property) = &key.property else {
                            return None;
                        };
                        Some(ReturnedMemberFunctionBinding {
                            target: ReturnedMemberFunctionBindingTarget::Prototype,
                            property: property.clone(),
                            binding: binding.clone(),
                        })
                    }
                    _ => None,
                })
                .collect(),
            Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
                if let Some(object_binding) =
                    self.resolve_returned_object_binding_from_call(callee, arguments)
                {
                    let bindings = object_binding
                        .string_properties
                        .iter()
                        .filter_map(|(property, value)| {
                            let binding = self.resolve_function_binding_from_expression(value)?;
                            Some(ReturnedMemberFunctionBinding {
                                target: ReturnedMemberFunctionBindingTarget::Value,
                                property: property.clone(),
                                binding,
                            })
                        })
                        .collect::<Vec<_>>();
                    if !bindings.is_empty() {
                        return bindings;
                    }
                }
                let Some(user_function) = self.resolve_user_function_from_expression(callee) else {
                    return Vec::new();
                };
                user_function.returned_member_function_bindings.clone()
            }
            Expression::Object(entries) => self.object_literal_member_function_bindings(entries),
            _ => Vec::new(),
        }
    }

    pub(in crate::backend::direct_wasm) fn copy_member_bindings_for_alias(
        &mut self,
        name: &str,
        source_name: &str,
    ) {
        let mut function_bindings = Vec::new();
        let mut function_capture_slots = Vec::new();
        let mut getter_bindings = Vec::new();
        let mut setter_bindings = Vec::new();

        for (key, binding) in self
            .member_function_bindings
            .iter()
            .chain(self.module.global_member_function_bindings.iter())
        {
            let target = match &key.target {
                MemberFunctionBindingTarget::Identifier(target) if target == source_name => {
                    Some(MemberFunctionBindingTarget::Identifier(name.to_string()))
                }
                MemberFunctionBindingTarget::Prototype(target) if target == source_name => {
                    Some(MemberFunctionBindingTarget::Prototype(name.to_string()))
                }
                _ => None,
            };
            if let Some(target) = target {
                let rebound_key = MemberFunctionBindingKey {
                    target,
                    property: key.property.clone(),
                };
                function_bindings.push((rebound_key.clone(), binding.clone()));
                if let Some(capture_slots) = self
                    .member_function_capture_slots
                    .get(key)
                    .cloned()
                    .or_else(|| {
                        self.module
                            .global_member_function_capture_slots
                            .get(key)
                            .cloned()
                    })
                {
                    function_capture_slots.push((rebound_key, capture_slots));
                }
            }
        }

        for (key, binding) in self
            .member_getter_bindings
            .iter()
            .chain(self.module.global_member_getter_bindings.iter())
        {
            let target = match &key.target {
                MemberFunctionBindingTarget::Identifier(target) if target == source_name => {
                    Some(MemberFunctionBindingTarget::Identifier(name.to_string()))
                }
                MemberFunctionBindingTarget::Prototype(target) if target == source_name => {
                    Some(MemberFunctionBindingTarget::Prototype(name.to_string()))
                }
                _ => None,
            };
            if let Some(target) = target {
                getter_bindings.push((
                    MemberFunctionBindingKey {
                        target,
                        property: key.property.clone(),
                    },
                    binding.clone(),
                ));
            }
        }

        for (key, binding) in self
            .member_setter_bindings
            .iter()
            .chain(self.module.global_member_setter_bindings.iter())
        {
            let target = match &key.target {
                MemberFunctionBindingTarget::Identifier(target) if target == source_name => {
                    Some(MemberFunctionBindingTarget::Identifier(name.to_string()))
                }
                MemberFunctionBindingTarget::Prototype(target) if target == source_name => {
                    Some(MemberFunctionBindingTarget::Prototype(name.to_string()))
                }
                _ => None,
            };
            if let Some(target) = target {
                setter_bindings.push((
                    MemberFunctionBindingKey {
                        target,
                        property: key.property.clone(),
                    },
                    binding.clone(),
                ));
            }
        }

        for (key, binding) in function_bindings {
            self.member_function_bindings
                .insert(key.clone(), binding.clone());
            if self.binding_name_is_global(name) {
                self.module
                    .global_member_function_bindings
                    .insert(key, binding);
            }
        }
        for (key, capture_slots) in function_capture_slots {
            self.member_function_capture_slots
                .insert(key.clone(), capture_slots.clone());
            if self.binding_name_is_global(name) {
                self.module
                    .global_member_function_capture_slots
                    .insert(key, capture_slots);
            }
        }
        for (key, binding) in getter_bindings {
            self.member_getter_bindings
                .insert(key.clone(), binding.clone());
            if self.binding_name_is_global(name) {
                self.module
                    .global_member_getter_bindings
                    .insert(key, binding);
            }
        }
        for (key, binding) in setter_bindings {
            self.member_setter_bindings
                .insert(key.clone(), binding.clone());
            if self.binding_name_is_global(name) {
                self.module
                    .global_member_setter_bindings
                    .insert(key, binding);
            }
        }
    }

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
        let value_kind = self
            .infer_value_kind(value)
            .unwrap_or(StaticValueKind::Unknown);
        self.local_kinds.insert(name.to_string(), value_kind);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn initialize_returned_member_capture_slots_for_value(
        &mut self,
        name: &str,
        value: &Expression,
        value_local: u32,
    ) -> DirectResult<HashMap<String, BTreeMap<String, String>>> {
        let (callee, arguments) = match value {
            Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
                (callee.as_ref(), arguments.as_slice())
            }
            _ => return Ok(HashMap::new()),
        };
        let LocalFunctionBinding::User(function_name) = self
            .resolve_function_binding_from_expression(callee)
            .unwrap_or(LocalFunctionBinding::Builtin(String::new()))
        else {
            return Ok(HashMap::new());
        };
        let Some(user_function) = self.module.user_function_map.get(&function_name).cloned() else {
            return Ok(HashMap::new());
        };
        if user_function.returned_member_function_bindings.is_empty() {
            return Ok(HashMap::new());
        }
        let Some(function) = self
            .resolve_registered_function_declaration(&user_function.name)
            .cloned()
        else {
            return Ok(HashMap::new());
        };
        let Some(returned_identifier) = collect_returned_identifier(&function.body) else {
            return Ok(HashMap::new());
        };
        let local_aliases = collect_returned_member_local_aliases(&function.body);
        let mut initialized_slots: BTreeMap<String, String> = BTreeMap::new();
        let mut property_slots: HashMap<String, BTreeMap<String, String>> = HashMap::new();

        for binding in &user_function.returned_member_function_bindings {
            let LocalFunctionBinding::User(member_function_name) = &binding.binding else {
                continue;
            };
            let Some(captures) = self
                .module
                .user_function_capture_bindings
                .get(member_function_name)
                .cloned()
            else {
                continue;
            };
            let mut capture_slots = BTreeMap::new();
            for capture_name in captures.keys() {
                let slot_name = if let Some(existing) = initialized_slots.get(capture_name) {
                    existing.clone()
                } else {
                    let (source_expression, source_uses_value_local) =
                        if capture_name == &returned_identifier {
                            (value.clone(), true)
                        } else if let Some(alias) = local_aliases.get(capture_name) {
                            (
                                self.substitute_user_function_argument_bindings(
                                    alias,
                                    &user_function,
                                    arguments,
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
                                    arguments,
                                ),
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
                        self.capture_slot_source_bindings
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
        let (callee, arguments) = match value {
            Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
                (callee.as_ref(), arguments.as_slice())
            }
            _ => return None,
        };
        let LocalFunctionBinding::User(function_name) =
            self.resolve_function_binding_from_expression(callee)?
        else {
            return None;
        };
        let user_function = self.module.user_function_map.get(&function_name)?;
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
                .module
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
                    self.substitute_user_function_argument_bindings(alias, user_function, arguments)
                } else if let Some(param_name) = user_function.params.iter().find(|param| {
                    *param == capture_name
                        || scoped_binding_source_name(param)
                            .is_some_and(|source_name| source_name == capture_name)
                }) {
                    self.substitute_user_function_argument_bindings(
                        &Expression::Identifier(param_name.clone()),
                        user_function,
                        arguments,
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

    pub(in crate::backend::direct_wasm) fn insert_inherited_member_function_binding_for_name(
        &mut self,
        name: &str,
        binding: ReturnedMemberFunctionBinding,
        capture_slots_by_property: &HashMap<String, BTreeMap<String, String>>,
    ) {
        let property_name = binding.property.clone();
        let target = match binding.target {
            ReturnedMemberFunctionBindingTarget::Value => {
                MemberFunctionBindingTarget::Identifier(name.to_string())
            }
            ReturnedMemberFunctionBindingTarget::Prototype => {
                MemberFunctionBindingTarget::Prototype(name.to_string())
            }
        };
        let key = MemberFunctionBindingKey {
            target,
            property: MemberFunctionBindingProperty::String(property_name.clone()),
        };
        self.member_function_bindings
            .insert(key.clone(), binding.binding.clone());
        if let Some(capture_slots) = capture_slots_by_property.get(&property_name).cloned() {
            self.member_function_capture_slots
                .insert(key.clone(), capture_slots.clone());
            if self.binding_name_is_global(name) {
                self.module
                    .global_member_function_capture_slots
                    .insert(key.clone(), capture_slots);
            }
        }
        if self.binding_name_is_global(name) {
            self.module
                .global_member_function_bindings
                .insert(key, binding.binding);
        }
    }

    pub(in crate::backend::direct_wasm) fn update_member_function_bindings_for_value(
        &mut self,
        name: &str,
        value: &Expression,
        value_local: u32,
    ) -> DirectResult<()> {
        self.clear_member_function_bindings_for_name(name);
        if let Expression::Identifier(source_name) = value {
            self.copy_member_bindings_for_alias(name, source_name);
            return Ok(());
        }

        let inherited_source = self
            .direct_iterator_binding_source_expression(value)
            .unwrap_or(value);
        if let Expression::Identifier(source_name) = inherited_source {
            self.copy_member_bindings_for_alias(name, source_name);
            return Ok(());
        }
        let capture_slots_by_property = self.initialize_returned_member_capture_slots_for_value(
            name,
            inherited_source,
            value_local,
        )?;
        for binding in self.inherited_member_function_bindings(inherited_source) {
            self.insert_inherited_member_function_binding_for_name(
                name,
                binding,
                &capture_slots_by_property,
            );
        }
        if let Expression::GetIterator(iterated) = value {
            let iterator_call = Expression::Call {
                callee: Box::new(Expression::Member {
                    object: Box::new((**iterated).clone()),
                    property: Box::new(symbol_iterator_expression()),
                }),
                arguments: Vec::new(),
            };
            let iterator_capture_slots_by_property = self
                .initialize_returned_member_capture_slots_for_value(
                    name,
                    &iterator_call,
                    value_local,
                )?;
            for binding in self.inherited_member_function_bindings(&iterator_call) {
                self.insert_inherited_member_function_binding_for_name(
                    name,
                    binding,
                    &iterator_capture_slots_by_property,
                );
            }
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn update_object_literal_member_bindings_for_value(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Expression::Object(entries) = value else {
            return;
        };

        self.clear_object_literal_member_bindings_for_name(name);

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
                crate::ir::hir::ObjectEntry::Data { key, value } => {
                    (key, self.resolve_function_binding_from_expression(value), 0)
                }
                crate::ir::hir::ObjectEntry::Getter { key, getter } => (
                    key,
                    self.resolve_function_binding_from_expression(getter),
                    1,
                ),
                crate::ir::hir::ObjectEntry::Setter { key, setter } => (
                    key,
                    self.resolve_function_binding_from_expression(setter),
                    2,
                ),
                crate::ir::hir::ObjectEntry::Spread(_) => return,
            };

            let materialized_key = self
                .resolve_property_key_expression(key)
                .unwrap_or_else(|| self.materialize_static_expression(key));
            let Some(property_name) = self.member_function_binding_property(&materialized_key)
            else {
                continue;
            };
            let state = states.entry(property_name).or_insert((None, None, None));
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
                self.member_function_bindings
                    .insert(key.clone(), binding.clone());
                if self.binding_name_is_global(name) {
                    self.module
                        .global_member_function_bindings
                        .insert(key.clone(), binding);
                }
            }
            if let Some(binding) = getter_binding {
                self.member_getter_bindings
                    .insert(key.clone(), binding.clone());
                if self.binding_name_is_global(name) {
                    self.module
                        .global_member_getter_bindings
                        .insert(key.clone(), binding);
                }
            }
            if let Some(binding) = setter_binding {
                self.member_setter_bindings
                    .insert(key.clone(), binding.clone());
                if self.binding_name_is_global(name) {
                    self.module
                        .global_member_setter_bindings
                        .insert(key, binding);
                }
            }
        }
    }
}
