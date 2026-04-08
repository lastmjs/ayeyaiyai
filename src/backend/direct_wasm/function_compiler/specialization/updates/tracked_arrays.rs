use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn update_tracked_array_specialized_function_value(
        &mut self,
        name: &str,
        index: u32,
        value: &Expression,
    ) -> DirectResult<()> {
        self.state
            .speculation
            .static_semantics
            .clear_tracked_array_specialized_function_value(name, index);
        let Some(specialized) = self.resolve_updated_specialized_function_value(value)? else {
            return Ok(());
        };
        self.state
            .speculation
            .static_semantics
            .set_tracked_array_specialized_function_value(name, index, specialized);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn resolve_tracked_array_specialized_function_value(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<SpecializedFunctionValue> {
        let Expression::Identifier(name) = object else {
            return None;
        };
        let index = argument_index_from_expression(property)?;
        self.state
            .speculation
            .static_semantics
            .tracked_array_specialized_function_value(name, index)
    }
}
