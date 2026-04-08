use super::*;

#[derive(Clone, Default)]
pub(in crate::backend::direct_wasm) struct UserFunctionParameterAnalysis {
    pub(in crate::backend::direct_wasm) function_bindings_by_function:
        HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
    pub(in crate::backend::direct_wasm) value_bindings_by_function:
        HashMap<String, HashMap<String, Option<Expression>>>,
    pub(in crate::backend::direct_wasm) array_bindings_by_function:
        HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
    pub(in crate::backend::direct_wasm) object_bindings_by_function:
        HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
}

impl UserFunctionParameterAnalysis {
    pub(in crate::backend::direct_wasm) fn clear(&mut self) {
        self.function_bindings_by_function.clear();
        self.value_bindings_by_function.clear();
        self.array_bindings_by_function.clear();
        self.object_bindings_by_function.clear();
    }

    pub(in crate::backend::direct_wasm) fn bindings_for(
        &self,
        function_name: &str,
    ) -> PreparedFunctionParameterBindings {
        PreparedFunctionParameterBindings {
            function_bindings: self
                .function_bindings_by_function
                .get(function_name)
                .cloned()
                .unwrap_or_default(),
            value_bindings: self
                .value_bindings_by_function
                .get(function_name)
                .cloned()
                .unwrap_or_default(),
            array_bindings: self
                .array_bindings_by_function
                .get(function_name)
                .cloned()
                .unwrap_or_default(),
            object_bindings: self
                .object_bindings_by_function
                .get(function_name)
                .cloned()
                .unwrap_or_default(),
        }
    }
}

#[derive(Clone, Default)]
pub(in crate::backend::direct_wasm) struct PreparedFunctionParameterBindings {
    pub(in crate::backend::direct_wasm) function_bindings:
        HashMap<String, Option<LocalFunctionBinding>>,
    pub(in crate::backend::direct_wasm) value_bindings: HashMap<String, Option<Expression>>,
    pub(in crate::backend::direct_wasm) array_bindings: HashMap<String, Option<ArrayValueBinding>>,
    pub(in crate::backend::direct_wasm) object_bindings:
        HashMap<String, Option<ObjectValueBinding>>,
}
