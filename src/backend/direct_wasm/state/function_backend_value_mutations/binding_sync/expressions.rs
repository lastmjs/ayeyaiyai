use super::*;

impl<'a> FunctionCompilerBackend<'a> {
    pub(in crate::backend::direct_wasm) fn set_global_expression_binding(
        &mut self,
        name: &str,
        value: Expression,
    ) {
        self.global_semantics
            .values
            .set_value_binding(name.to_string(), value);
    }

    pub(in crate::backend::direct_wasm) fn sync_global_expression_binding(
        &mut self,
        name: &str,
        value: Option<Expression>,
    ) {
        if let Some(value) = value {
            self.set_global_expression_binding(name, value);
        } else {
            self.global_semantics.values.clear_value_binding(name);
        }
    }
}
