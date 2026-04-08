use super::*;

#[derive(Clone, Default)]
pub(in crate::backend::direct_wasm) struct GlobalFunctionService {
    pub(in crate::backend::direct_wasm) function_bindings: HashMap<String, LocalFunctionBinding>,
    pub(in crate::backend::direct_wasm) specialized_function_values:
        HashMap<String, SpecializedFunctionValue>,
}

impl GlobalFunctionService {
    pub(in crate::backend::direct_wasm) fn reset_for_program(&mut self) {
        self.function_bindings.clear();
        self.specialized_function_values.clear();
    }

    pub(in crate::backend::direct_wasm) fn set_function_binding(
        &mut self,
        name: &str,
        binding: LocalFunctionBinding,
    ) {
        self.function_bindings.insert(name.to_string(), binding);
    }

    pub(in crate::backend::direct_wasm) fn function_binding(
        &self,
        name: &str,
    ) -> Option<&LocalFunctionBinding> {
        self.function_bindings.get(name)
    }

    pub(in crate::backend::direct_wasm) fn clear_function_binding(&mut self, name: &str) {
        self.function_bindings.remove(name);
    }

    pub(in crate::backend::direct_wasm) fn clear_specialized_function_value(&mut self, name: &str) {
        self.specialized_function_values.remove(name);
    }

    pub(in crate::backend::direct_wasm) fn find_user_function_binding_name(
        &self,
        function_name: &str,
    ) -> Option<String> {
        self.function_bindings
            .iter()
            .find_map(|(binding_name, binding)| match binding {
                LocalFunctionBinding::User(bound_name) if bound_name == function_name => {
                    Some(binding_name.clone())
                }
                _ => None,
            })
    }
}
