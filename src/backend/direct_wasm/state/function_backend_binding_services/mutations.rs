use super::super::*;

impl<'a> FunctionCompilerBackend<'a> {
    pub(in crate::backend::direct_wasm) fn set_global_string_binding(
        &mut self,
        name: &str,
        text: String,
    ) {
        self.set_global_expression_binding(name, Expression::String(text));
        self.set_global_binding_kind(name, StaticValueKind::String);
    }

    pub(in crate::backend::direct_wasm) fn set_global_number_binding(
        &mut self,
        name: &str,
        number: f64,
    ) {
        self.set_global_expression_binding(name, Expression::Number(number));
        self.set_global_binding_kind(name, StaticValueKind::Number);
    }

    pub(in crate::backend::direct_wasm) fn set_global_binding_kind(
        &mut self,
        name: &str,
        kind: StaticValueKind,
    ) {
        self.global_semantics.set_global_binding_kind(name, kind);
    }

    pub(in crate::backend::direct_wasm) fn set_global_function_binding(
        &mut self,
        name: &str,
        binding: LocalFunctionBinding,
    ) {
        self.global_semantics
            .set_global_function_binding(name, binding);
        self.set_global_binding_kind(name, StaticValueKind::Function);
    }

    pub(in crate::backend::direct_wasm) fn set_global_user_function_reference(
        &mut self,
        name: &str,
    ) {
        self.set_global_binding_kind(name, StaticValueKind::Function);
        self.set_global_expression_binding(name, Expression::Identifier(name.to_string()));
        self.set_global_function_binding(name, LocalFunctionBinding::User(name.to_string()));
    }

    pub(in crate::backend::direct_wasm) fn sync_global_function_binding(
        &mut self,
        name: &str,
        binding: Option<LocalFunctionBinding>,
    ) {
        if let Some(binding) = binding {
            self.set_global_function_binding(name, binding);
        } else {
            self.global_semantics.clear_global_function_binding(name);
        }
    }
}
