use super::FunctionValueSemanticsState;
use crate::backend::direct_wasm::LocalFunctionBinding;

impl FunctionValueSemanticsState {
    pub(in crate::backend::direct_wasm) fn local_function_binding(
        &self,
        name: &str,
    ) -> Option<&LocalFunctionBinding> {
        self.local_function_bindings.get(name)
    }

    pub(in crate::backend::direct_wasm) fn set_local_function_binding(
        &mut self,
        name: &str,
        binding: LocalFunctionBinding,
    ) {
        self.local_function_bindings
            .insert(name.to_string(), binding);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_function_binding(&mut self, name: &str) {
        self.local_function_bindings.remove(name);
    }
}
