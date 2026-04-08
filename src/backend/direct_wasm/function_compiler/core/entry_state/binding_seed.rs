use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn prepare_binding_state(
        module: &DirectWasmCompiler,
        user_function: Option<&UserFunction>,
        declaration: Option<&FunctionDeclaration>,
        total_param_count: u32,
        next_local_index: &mut u32,
        global_binding_environment: &GlobalBindingEnvironment,
        parameter_names: &[String],
        parameter_bindings: &HashMap<String, Option<LocalFunctionBinding>>,
        parameter_value_bindings: &HashMap<String, Option<Expression>>,
        parameter_array_bindings: &HashMap<String, Option<ArrayValueBinding>>,
        parameter_object_bindings: &HashMap<String, Option<ObjectValueBinding>>,
    ) -> EntryBindingState {
        let mut bindings = Self::seed_parameter_binding_state(
            module,
            global_binding_environment,
            parameter_names,
            parameter_bindings,
            parameter_value_bindings,
            parameter_array_bindings,
            parameter_object_bindings,
        );
        Self::add_fallback_local(&mut bindings, total_param_count);
        Self::allocate_function_scope_locals(&mut bindings, user_function, next_local_index);
        Self::apply_special_function_bindings(&mut bindings, user_function, declaration);
        bindings
    }

    fn seed_parameter_binding_state(
        module: &DirectWasmCompiler,
        global_binding_environment: &GlobalBindingEnvironment,
        parameter_names: &[String],
        parameter_bindings: &HashMap<String, Option<LocalFunctionBinding>>,
        parameter_value_bindings: &HashMap<String, Option<Expression>>,
        parameter_array_bindings: &HashMap<String, Option<ArrayValueBinding>>,
        parameter_object_bindings: &HashMap<String, Option<ObjectValueBinding>>,
    ) -> EntryBindingState {
        let mut locals = HashMap::new();
        let mut local_kinds = HashMap::new();
        let mut local_function_bindings = HashMap::new();
        for (index, param) in parameter_names.iter().enumerate() {
            if !locals.contains_key(param) {
                locals.insert(param.clone(), index as u32);
            }
            local_kinds.insert(param.clone(), StaticValueKind::Unknown);
            if let Some(Some(binding)) = parameter_bindings.get(param) {
                local_function_bindings.insert(param.clone(), binding.clone());
            }
        }

        let mut local_value_bindings = HashMap::new();
        for param in parameter_names {
            if let Some(Some(binding)) = parameter_value_bindings.get(param) {
                local_value_bindings.insert(param.clone(), binding.clone());
            }
        }

        let mut local_array_bindings = HashMap::new();
        for param in parameter_names {
            if let Some(Some(binding)) = parameter_array_bindings.get(param) {
                local_array_bindings.insert(param.clone(), binding.clone());
            }
        }

        let mut local_object_bindings = HashMap::new();
        for param in parameter_names {
            if let Some(Some(binding)) = parameter_object_bindings.get(param) {
                local_object_bindings.insert(param.clone(), binding.clone());
                local_kinds.insert(param.clone(), StaticValueKind::Object);
                continue;
            }
            if let Some(Some(binding)) = parameter_value_bindings.get(param) {
                let resolved_binding = module
                    .materialize_global_expression_with_state(
                        binding,
                        &HashMap::new(),
                        &global_binding_environment.value_bindings,
                        &global_binding_environment.object_bindings,
                    )
                    .unwrap_or_else(|| module.materialize_global_expression(binding));
                if let Some(object_binding) = module.infer_global_object_binding(&resolved_binding)
                {
                    local_object_bindings.insert(param.clone(), object_binding);
                    local_kinds.insert(param.clone(), StaticValueKind::Object);
                }
            }
        }

        EntryBindingState {
            locals,
            static_bindings: PreparedLocalStaticBindings {
                local_kinds,
                local_value_bindings,
                local_function_bindings,
                local_array_bindings,
                local_object_bindings,
            },
        }
    }

    fn add_fallback_local(bindings: &mut EntryBindingState, total_param_count: u32) {
        let fallback_local_name = "__ayy_fallback_local";
        bindings
            .locals
            .insert(fallback_local_name.to_string(), total_param_count);
        bindings
            .static_bindings
            .local_kinds
            .insert(fallback_local_name.to_string(), StaticValueKind::Unknown);
    }

    fn allocate_function_scope_locals(
        bindings: &mut EntryBindingState,
        user_function: Option<&UserFunction>,
        next_local_index: &mut u32,
    ) {
        if let Some(user_function) = user_function {
            let mut scope_bindings = user_function
                .scope_bindings
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            scope_bindings.sort();
            for binding in scope_bindings {
                if binding == "arguments" || bindings.locals.contains_key(&binding) {
                    continue;
                }
                bindings.locals.insert(binding.clone(), *next_local_index);
                bindings
                    .static_bindings
                    .local_kinds
                    .insert(binding, StaticValueKind::Unknown);
                *next_local_index += 1;
            }
        }
    }
}
