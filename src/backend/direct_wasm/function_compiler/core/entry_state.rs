use super::*;

#[path = "entry_state/binding_seed.rs"]
mod binding_seed;
#[path = "entry_state/execution_context.rs"]
mod execution_context;
#[path = "entry_state/function_bindings.rs"]
mod function_bindings;
#[path = "entry_state/parameter_layout.rs"]
mod parameter_layout;
#[path = "entry_state/support_locals.rs"]
mod support_locals;

struct EntryBindingState {
    locals: HashMap<String, u32>,
    static_bindings: PreparedLocalStaticBindings,
}

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn prepare_top_level_entry_state(
        module: &mut DirectWasmCompiler,
        strict_mode: bool,
        global_binding_environment: &GlobalBindingEnvironment,
    ) -> DirectResult<PreparedFunctionEntryState> {
        let empty_function_bindings = HashMap::new();
        let empty_value_bindings = HashMap::new();
        let empty_array_bindings = HashMap::new();
        let empty_object_bindings = HashMap::new();
        Self::prepare_entry_state(
            module,
            FunctionCompilationRequest {
                user_function: None,
                declaration: None,
                behavior: FunctionCompilerBehavior {
                    allow_return: false,
                    mapped_arguments: false,
                    strict_mode,
                },
                global_binding_environment,
                parameter_bindings: FunctionParameterBindingView::new(
                    &empty_function_bindings,
                    &empty_value_bindings,
                    &empty_array_bindings,
                    &empty_object_bindings,
                ),
            },
        )
    }

    pub(in crate::backend::direct_wasm) fn prepare_user_function_entry_state(
        module: &mut DirectWasmCompiler,
        declaration: &FunctionDeclaration,
        user_function: &UserFunction,
        parameter_bindings: &HashMap<String, Option<LocalFunctionBinding>>,
        parameter_value_bindings: &HashMap<String, Option<Expression>>,
        parameter_array_bindings: &HashMap<String, Option<ArrayValueBinding>>,
        parameter_object_bindings: &HashMap<String, Option<ObjectValueBinding>>,
        global_binding_environment: &GlobalBindingEnvironment,
    ) -> DirectResult<PreparedFunctionEntryState> {
        Self::prepare_entry_state(
            module,
            FunctionCompilationRequest {
                user_function: Some(user_function),
                declaration: Some(declaration),
                behavior: FunctionCompilerBehavior {
                    allow_return: true,
                    mapped_arguments: declaration.mapped_arguments,
                    strict_mode: declaration.strict,
                },
                global_binding_environment,
                parameter_bindings: FunctionParameterBindingView::new(
                    parameter_bindings,
                    parameter_value_bindings,
                    parameter_array_bindings,
                    parameter_object_bindings,
                ),
            },
        )
    }

    #[cfg(test)]
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
        let declaration = user_function.and_then(|function| {
            module
                .state
                .function_registry
                .catalog
                .registered_function(&function.name)
                .cloned()
        });
        let global_binding_environment = module.state.snapshot_global_binding_environment();
        let entry_state = Self::prepare_entry_state(
            module,
            FunctionCompilationRequest {
                user_function,
                declaration: declaration.as_ref(),
                behavior: FunctionCompilerBehavior {
                    allow_return,
                    mapped_arguments,
                    strict_mode,
                },
                global_binding_environment: &global_binding_environment,
                parameter_bindings: FunctionParameterBindingView::new(
                    parameter_bindings,
                    parameter_value_bindings,
                    parameter_array_bindings,
                    parameter_object_bindings,
                ),
            },
        )?;
        let prepared_analysis = PreparedProgramAnalysis::new(
            HashMap::new(),
            module.state.prepared_user_function_metadata_snapshot(),
            module
                .state
                .user_functions()
                .iter()
                .map(|function| function.name.clone())
                .collect(),
            module
                .state
                .function_registry
                .eval_local_function_bindings_snapshot(),
            module
                .state
                .function_registry
                .user_function_capture_bindings_snapshot(),
            module.state.snapshot_global_binding_environment(),
            module.state.snapshot_global_static_semantics(),
        );
        Self::from_prepared_entry_state(
            module,
            entry_state,
            prepared_analysis.function_compiler_inputs(),
        )
    }

    pub(in crate::backend::direct_wasm) fn prepare_entry_state<'b>(
        module: &mut DirectWasmCompiler,
        request: FunctionCompilationRequest<'b>,
    ) -> DirectResult<PreparedFunctionEntryState> {
        let FunctionCompilationRequest {
            user_function,
            declaration,
            behavior,
            global_binding_environment,
            parameter_bindings,
        } = request;
        let FunctionParameterBindingView {
            function_bindings: parameter_bindings,
            value_bindings: parameter_value_bindings,
            array_bindings: parameter_array_bindings,
            object_bindings: parameter_object_bindings,
        } = parameter_bindings;
        let FunctionCompilerBehavior {
            allow_return,
            mapped_arguments,
            strict_mode,
        } = behavior;

        let (
            visible_param_count,
            total_param_count,
            actual_argument_count_local,
            extra_argument_param_locals,
            needs_parameter_scope_arguments_local,
            parameter_names,
            parameter_defaults,
        ) = Self::reserve_parameter_layout(user_function);
        let mut next_local_index = total_param_count + 3;
        let bindings = Self::prepare_binding_state(
            module,
            user_function,
            declaration,
            total_param_count,
            &mut next_local_index,
            global_binding_environment,
            &parameter_names,
            parameter_bindings,
            parameter_value_bindings,
            parameter_array_bindings,
            parameter_object_bindings,
        );
        let throw_tag_local = total_param_count + 1;
        let throw_value_local = total_param_count + 2;
        let (parameter_scope_arguments_local, parameter_initialized_locals) =
            Self::allocate_parameter_support_locals(
                &parameter_names,
                !parameter_defaults.is_empty(),
                needs_parameter_scope_arguments_local,
                &mut next_local_index,
            );
        let execution_context =
            Self::prepare_execution_context(&bindings, user_function, declaration, strict_mode);

        Ok(PreparedFunctionEntryState {
            parameter_state: PreparedFunctionParameterState {
                parameter_names,
                parameter_defaults,
                parameter_initialized_locals,
                parameter_scope_arguments_local,
                param_count: total_param_count,
                visible_param_count,
                actual_argument_count_local,
                extra_argument_param_locals,
                mapped_arguments,
            },
            runtime: PreparedFunctionRuntimeState {
                locals: bindings.locals,
                throw_tag_local,
                throw_value_local,
                next_local_index,
                allow_return,
            },
            static_bindings: bindings.static_bindings,
            execution_context,
        })
    }

    pub(in crate::backend::direct_wasm) fn from_prepared_entry_state(
        module: &'a mut DirectWasmCompiler,
        entry_state: PreparedFunctionEntryState,
        prepared_inputs: PreparedFunctionCompilerInputs,
    ) -> DirectResult<Self> {
        let program_context = prepared_inputs.shared_program_context();
        let global_static_semantics = program_context.required_global_static_semantics().clone();
        let assigned_nonlocal_binding_results =
            prepared_inputs.assigned_nonlocal_binding_results_snapshot();
        let CompilerState {
            module_artifacts,
            function_registry,
            global_semantics: _,
            test262,
        } = &mut module.state;
        Ok(Self {
            backend: FunctionCompilerBackend::new(
                module_artifacts,
                function_registry,
                test262,
                global_static_semantics,
            ),
            prepared_program: program_context,
            assigned_nonlocal_binding_results,
            state: FunctionCompilerState::from_prepared_entry_state(entry_state),
        })
    }
}
