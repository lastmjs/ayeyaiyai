use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn register_global_bindings(
        &mut self,
        statements: &[Statement],
    ) {
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

    pub(in crate::backend::direct_wasm) fn register_global_function_bindings(
        &mut self,
        functions: &[FunctionDeclaration],
    ) {
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

    pub(in crate::backend::direct_wasm) fn register_user_function_capture_bindings(
        &mut self,
        functions: &[FunctionDeclaration],
    ) {
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

    pub(in crate::backend::direct_wasm) fn reserve_function_constructor_implicit_global_bindings(
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

    pub(in crate::backend::direct_wasm) fn ensure_implicit_global_binding(
        &mut self,
        name: &str,
    ) -> ImplicitGlobalBinding {
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

    pub(in crate::backend::direct_wasm) fn next_allocated_global_index(&self) -> u32 {
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

    pub(in crate::backend::direct_wasm) fn reserve_global_runtime_prototype_binding_globals(
        &mut self,
    ) {
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

    pub(in crate::backend::direct_wasm) fn update_user_function_home_object_binding(
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

    pub(in crate::backend::direct_wasm) fn update_global_object_literal_home_bindings(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
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

    pub(in crate::backend::direct_wasm) fn clear_global_object_literal_member_bindings_for_name(
        &mut self,
        name: &str,
    ) {
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

    pub(in crate::backend::direct_wasm) fn update_global_object_literal_member_bindings_for_value(
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

    pub(in crate::backend::direct_wasm) fn update_global_object_prototype_binding(
        &mut self,
        name: &str,
        prototype: &Expression,
    ) {
        self.global_object_prototype_bindings.insert(
            name.to_string(),
            self.materialize_global_expression(prototype),
        );
    }

    pub(in crate::backend::direct_wasm) fn update_global_object_prototype_binding_from_value(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        if let Some(prototype) = object_literal_prototype_expression(value) {
            self.update_global_object_prototype_binding(name, &prototype);
        }
    }

    pub(in crate::backend::direct_wasm) fn record_global_runtime_prototype_variant(
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

    pub(in crate::backend::direct_wasm) fn update_global_expression_metadata(
        &mut self,
        expression: &Expression,
    ) {
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
}
