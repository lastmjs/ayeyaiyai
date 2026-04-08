use super::*;

impl<'a> ProgramCompilationSession<'a> {
    pub(super) fn run_function_discovery_phase(&mut self, program: &Program) -> DirectResult<()> {
        self.compiler.register_functions(&program.functions)?;
        self.compiler.register_static_eval_functions(program)?;
        Ok(())
    }

    pub(super) fn run_global_binding_phase(&mut self, program: &Program) {
        self.compiler.register_global_bindings(&program.statements);
        self.compiler
            .register_global_function_bindings(&program.functions);
        self.compiler
            .reserve_global_array_runtime_state_bindings(program);
    }

    pub(super) fn run_parameter_analysis_phase(&mut self, program: &Program) {
        self.compiler
            .apply_user_function_parameter_analysis(program);
    }

    pub(super) fn run_runtime_reservation_phase(&mut self, program: &Program) -> DirectResult<()> {
        self.compiler
            .register_user_function_capture_bindings(&program.functions);
        self.compiler
            .reserve_function_constructor_implicit_global_bindings(program)?;
        self.compiler
            .reserve_global_runtime_prototype_binding_globals();
        Ok(())
    }
}
