use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn register_static_eval_functions(
        &mut self,
        program: &Program,
    ) -> DirectResult<()> {
        self.register_static_eval_functions_in_statements(&program.statements, None)?;
        for function in &program.functions {
            self.register_static_eval_functions_in_statements(
                &function.body,
                Some(function.name.as_str()),
            )?;
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn register_static_eval_functions_in_statements(
        &mut self,
        statements: &[Statement],
        current_function_name: Option<&str>,
    ) -> DirectResult<()> {
        for statement in statements {
            self.register_static_eval_functions_in_statement(statement, current_function_name)?;
        }
        Ok(())
    }
}
