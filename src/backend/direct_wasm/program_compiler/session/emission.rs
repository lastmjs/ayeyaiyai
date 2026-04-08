use super::*;

impl<'a> ProgramCompilationSession<'a> {
    pub(super) fn capture_module_layout(&self) -> PreparedModuleLayout {
        self.compiler.capture_prepared_module_layout()
    }

    pub(super) fn compile_registered_functions(
        &mut self,
        prepared_program: &PreparedBackendProgram,
    ) -> DirectResult<Vec<CompiledFunction>> {
        prepared_program
            .user_functions
            .iter()
            .map(|function| {
                self.compiler.compile_user_function(
                    function,
                    prepared_program.analysis.function_compiler_inputs(),
                )
            })
            .collect()
    }

    pub(super) fn emit_program(
        &mut self,
        prepared_program: PreparedBackendProgram,
    ) -> DirectResult<EmittedBackendProgram> {
        let compiled_functions = self.compile_registered_functions(&prepared_program)?;
        let compiled_start = self.compiler.compile_start(
            &prepared_program.start,
            prepared_program.analysis.function_compiler_inputs(),
        )?;
        let (int_min_ptr, int_min_len) = self.compiler.intern_string(b"-2147483648".to_vec());
        let (string_data, next_data_offset) = self.compiler.snapshot_module_data();

        Ok(EmittedBackendProgram {
            compiled_start,
            compiled_functions,
            module_layout: prepared_program.module_layout,
            artifacts: EmittedModuleArtifacts {
                string_data,
                next_data_offset,
                int_min_ptr,
                int_min_len,
            },
        })
    }
}
