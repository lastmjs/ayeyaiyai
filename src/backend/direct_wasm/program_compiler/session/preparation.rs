use super::*;

impl<'a> ProgramCompilationSession<'a> {
    pub(super) fn prepare_program(
        &mut self,
        program: &Program,
    ) -> DirectResult<PreparedBackendProgram> {
        self.run_function_discovery_phase(program)?;
        self.run_global_binding_phase(program);
        self.run_parameter_analysis_phase(program);
        self.run_runtime_reservation_phase(program)?;
        let global_binding_environment = self.compiler.snapshot_global_binding_environment();
        let global_static_semantics = self.compiler.snapshot_global_static_semantics();

        let start = self.prepare_start_function(program, &global_binding_environment)?;
        let (user_functions, analysis) = self.prepare_user_function_compilations(
            program,
            &global_binding_environment,
            global_static_semantics,
        )?;

        Ok(PreparedBackendProgram {
            start,
            analysis,
            user_functions,
            module_layout: self.capture_module_layout(),
        })
    }

    pub(super) fn prepare_start_function(
        &mut self,
        program: &Program,
        global_binding_environment: &GlobalBindingEnvironment,
    ) -> DirectResult<PreparedStartFunction> {
        Ok(PreparedStartFunction {
            statements: self.compiler.prepare_start_statements(program),
            entry_state: FunctionCompiler::prepare_top_level_entry_state(
                self.compiler,
                program.strict,
                global_binding_environment,
            )?,
        })
    }

    pub(super) fn prepare_user_function_compilations(
        &mut self,
        program: &Program,
        global_binding_environment: &GlobalBindingEnvironment,
        global_static_semantics: GlobalStaticSemanticsSnapshot,
    ) -> DirectResult<(
        Vec<PreparedUserFunctionCompilation>,
        PreparedProgramAnalysis,
    )> {
        let mut user_functions = Vec::new();
        let mut ordered_user_function_names = Vec::new();
        let mut assigned_nonlocal_binding_results = HashMap::new();
        let mut user_function_metadata = HashMap::new();
        for declaration in &program.functions {
            let Some((prepared_function, prepared_results)) =
                self.prepare_user_function_compilation(declaration, global_binding_environment)?
            else {
                continue;
            };
            ordered_user_function_names.push(prepared_function.metadata.name.clone());
            user_function_metadata.insert(
                prepared_function.metadata.name.clone(),
                prepared_function.metadata.clone(),
            );
            if !prepared_results.is_empty() {
                assigned_nonlocal_binding_results
                    .insert(declaration.name.clone(), prepared_results);
            }
            user_functions.push(prepared_function);
        }
        let eval_local_function_bindings = self.compiler.prepared_eval_local_function_bindings();
        let user_function_capture_bindings =
            self.compiler.prepared_user_function_capture_bindings();
        Ok((
            user_functions,
            PreparedProgramAnalysis::new(
                assigned_nonlocal_binding_results,
                user_function_metadata,
                ordered_user_function_names,
                eval_local_function_bindings,
                user_function_capture_bindings,
                global_binding_environment.clone(),
                global_static_semantics,
            ),
        ))
    }

    pub(super) fn prepare_user_function_compilation(
        &mut self,
        declaration: &FunctionDeclaration,
        global_binding_environment: &GlobalBindingEnvironment,
    ) -> DirectResult<Option<(PreparedUserFunctionCompilation, HashMap<String, Expression>)>> {
        let Some(user_function) = self.compiler.prepared_user_function(&declaration.name) else {
            return Ok(None);
        };

        let parameter_bindings = self
            .compiler
            .prepared_user_function_parameter_bindings(&declaration.name);
        let entry_state = FunctionCompiler::prepare_user_function_entry_state(
            self.compiler,
            declaration,
            &user_function,
            &parameter_bindings.function_bindings,
            &parameter_bindings.value_bindings,
            &parameter_bindings.array_bindings,
            &parameter_bindings.object_bindings,
            global_binding_environment,
        )?;
        let (analysis, assigned_nonlocal_binding_results) =
            self.prepare_user_function_analysis(&user_function);

        Ok(Some((
            PreparedUserFunctionCompilation {
                metadata: PreparedFunctionMetadata {
                    name: declaration.name.clone(),
                    declaration: declaration.clone(),
                    user_function: user_function.clone(),
                },
                analysis,
                entry_state,
            },
            assigned_nonlocal_binding_results,
        )))
    }

    pub(super) fn prepare_user_function_analysis(
        &mut self,
        user_function: &UserFunction,
    ) -> (PreparedUserFunctionAnalysis, HashMap<String, Expression>) {
        let assigned_nonlocal_bindings = self
            .compiler
            .collect_user_function_assigned_nonlocal_bindings(user_function);
        let assigned_nonlocal_binding_results = self
            .compiler
            .capture_assigned_nonlocal_binding_results(&assigned_nonlocal_bindings);
        (
            PreparedUserFunctionAnalysis {
                assigned_nonlocal_bindings,
            },
            assigned_nonlocal_binding_results,
        )
    }
}
