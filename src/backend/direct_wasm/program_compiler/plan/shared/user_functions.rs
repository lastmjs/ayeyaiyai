use super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct PreparedSharedProgramContext {
    pub(in crate::backend::direct_wasm) user_function_metadata:
        Rc<HashMap<String, PreparedFunctionMetadata>>,
    pub(in crate::backend::direct_wasm) user_function_order: Rc<Vec<String>>,
    pub(in crate::backend::direct_wasm) eval_local_function_bindings:
        Rc<HashMap<String, HashMap<String, String>>>,
    pub(in crate::backend::direct_wasm) user_function_capture_bindings:
        Rc<HashMap<String, HashMap<String, String>>>,
    pub(in crate::backend::direct_wasm) globals: PreparedGlobalProgramContext,
}

impl PreparedSharedProgramContext {
    pub(in crate::backend::direct_wasm) fn user_function_metadata(
        &self,
        function_name: &str,
    ) -> Option<&PreparedFunctionMetadata> {
        self.user_function_metadata.get(function_name)
    }

    pub(in crate::backend::direct_wasm) fn user_function(
        &self,
        function_name: &str,
    ) -> Option<&UserFunction> {
        self.user_function_metadata(function_name)
            .map(|metadata| &metadata.user_function)
    }

    pub(in crate::backend::direct_wasm) fn user_function_declaration(
        &self,
        function_name: &str,
    ) -> Option<&FunctionDeclaration> {
        self.user_function_metadata(function_name)
            .map(|metadata| &metadata.declaration)
    }

    pub(in crate::backend::direct_wasm) fn ordered_user_functions(&self) -> Vec<UserFunction> {
        self.user_function_order
            .iter()
            .filter_map(|function_name| self.user_function(function_name).cloned())
            .collect()
    }

    pub(in crate::backend::direct_wasm) fn contains_user_function(&self, name: &str) -> bool {
        self.user_function_metadata.contains_key(name)
    }

    pub(in crate::backend::direct_wasm) fn resolve_user_function_by_binding_name(
        &self,
        name: &str,
    ) -> Option<&UserFunction> {
        if let Some(LocalFunctionBinding::User(function_name)) = self
            .globals
            .required_global_static_semantics()
            .global_functions()
            .function_binding(name)
        {
            return self.user_function(&function_name);
        }
        if is_internal_user_function_identifier(name) {
            return self.user_function(name);
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn user_function_capture_bindings(
        &self,
        function_name: &str,
    ) -> Option<&HashMap<String, String>> {
        self.user_function_capture_bindings.get(function_name)
    }

    pub(in crate::backend::direct_wasm) fn eval_local_function_bindings(
        &self,
        function_name: &str,
    ) -> Option<&HashMap<String, String>> {
        self.eval_local_function_bindings.get(function_name)
    }

    pub(in crate::backend::direct_wasm) fn required_shared_global_binding_environment(
        &self,
    ) -> &SharedGlobalBindingEnvironment {
        self.globals.required_shared_global_binding_environment()
    }

    pub(in crate::backend::direct_wasm) fn required_global_static_semantics(
        &self,
    ) -> &GlobalStaticSemanticsSnapshot {
        self.globals.required_global_static_semantics()
    }
}
