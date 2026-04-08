use super::*;

impl<'a> FunctionCompilerBackend<'a> {
    pub(in crate::backend::direct_wasm) fn sync_global_arguments_binding(
        &mut self,
        name: &str,
        binding: Option<ArgumentsValueBinding>,
    ) {
        self.global_semantics
            .values
            .sync_arguments_binding(name, binding);
    }

    pub(in crate::backend::direct_wasm) fn apply_global_arguments_binding_named_effect(
        &mut self,
        name: &str,
        property_name: &str,
        effect: ArgumentsPropertyEffect,
    ) -> bool {
        let Some(binding): Option<&mut ArgumentsValueBinding> = self
            .global_semantics
            .values
            .arguments_bindings
            .get_mut(name)
        else {
            return false;
        };
        binding.apply_named_effect(property_name, effect);
        true
    }
}
