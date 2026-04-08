use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn static_expressions_equal(
        &self,
        actual: &Expression,
        expected: &Expression,
    ) -> bool {
        if let (Some(actual_text), Some(expected_text)) = (
            self.resolve_static_string_value(actual),
            self.resolve_static_string_value(expected),
        ) {
            return actual_text == expected_text;
        }

        if let (Some(actual_number), Some(expected_number)) = (
            self.resolve_static_number_value(actual),
            self.resolve_static_number_value(expected),
        ) {
            return actual_number == expected_number;
        }

        self.materialize_static_expression(actual) == self.materialize_static_expression(expected)
    }

    pub(in crate::backend::direct_wasm) fn array_bindings_equal(
        &self,
        actual: &ArrayValueBinding,
        expected: &ArrayValueBinding,
    ) -> bool {
        actual.values.len() == expected.values.len()
            && actual.values.iter().zip(expected.values.iter()).all(
                |(actual_value, expected_value)| match (actual_value, expected_value) {
                    (None, None) => true,
                    (Some(actual_value), Some(expected_value)) => {
                        self.static_expressions_equal(actual_value, expected_value)
                    }
                    _ => false,
                },
            )
    }
}
