use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn instantiate_eval_local_functions(
        &mut self,
        declarations: &HashMap<String, String>,
    ) -> DirectResult<()> {
        for (binding_name, function_name) in declarations {
            let value_expression = Expression::Identifier(function_name.clone());
            let value_local = self.allocate_temp_local();
            self.emit_numeric_expression(&value_expression)?;
            self.push_local_set(value_local);
            if self.resolve_current_local_binding(binding_name).is_some()
                || self.backend.global_binding_index(binding_name).is_some()
                || self
                    .resolve_eval_local_function_hidden_name(binding_name)
                    .is_some()
            {
                self.emit_store_identifier_value_local(
                    binding_name,
                    &value_expression,
                    value_local,
                )?;
            }
        }
        Ok(())
    }
}
