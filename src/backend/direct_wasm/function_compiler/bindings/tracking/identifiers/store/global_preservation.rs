use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn preserve_exact_static_global_string_binding(
        &mut self,
        name: &str,
        exact_static_number: Option<f64>,
        static_string_value: Option<&String>,
    ) {
        if exact_static_number.is_none()
            && let Some(text) = static_string_value
        {
            self.backend.set_global_string_binding(name, text.clone());
        }
    }

    pub(super) fn preserve_static_global_function_binding(
        &mut self,
        name: &str,
        function_binding: Option<&LocalFunctionBinding>,
    ) {
        if let Some(binding) = function_binding.cloned() {
            self.backend.set_global_function_binding(name, binding);
        }
    }

    pub(in crate::backend::direct_wasm) fn preserve_exact_static_global_number_binding(
        &mut self,
        name: &str,
        value_expression: &Expression,
    ) {
        let Some(number) = self.resolve_static_number_value(value_expression) else {
            return;
        };
        if number.is_nan() {
            return;
        }
        if number.is_finite()
            && number.fract() == 0.0
            && !(number == 0.0 && number.is_sign_negative())
        {
            return;
        }
        self.backend.set_global_number_binding(name, number);
    }
}
