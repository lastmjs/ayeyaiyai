use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn instantiate_eval_global_functions(
        &mut self,
        functions: &[FunctionDeclaration],
    ) -> DirectResult<()> {
        for function in functions {
            if !function.register_global {
                continue;
            }
            let value_expression = Expression::Identifier(function.name.clone());
            self.backend
                .set_global_user_function_reference(&function.name);
            self.instantiate_eval_global_function_property_descriptor(&function.name);
            let value_local = self.allocate_temp_local();
            let Some(user_function) = self.user_function(&function.name) else {
                return Err(Unsupported("eval global function runtime value"));
            };
            self.push_i32_const(user_function_runtime_value(user_function));
            self.push_local_set(value_local);
            self.emit_store_identifier_value_local(&function.name, &value_expression, value_local)?;
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn register_eval_global_function_local_bindings(
        &mut self,
        functions: &[FunctionDeclaration],
    ) {
        for function in functions {
            if !function.register_global
                || self
                    .state
                    .runtime
                    .locals
                    .bindings
                    .contains_key(&function.name)
            {
                continue;
            }
            let next_local_index = self.state.runtime.locals.next_local_index;
            self.state
                .runtime
                .locals
                .bindings
                .insert(function.name.clone(), next_local_index);
            self.state
                .speculation
                .static_semantics
                .set_local_kind(&function.name, StaticValueKind::Unknown);
            self.state.runtime.locals.next_local_index += 1;
        }
    }
}
