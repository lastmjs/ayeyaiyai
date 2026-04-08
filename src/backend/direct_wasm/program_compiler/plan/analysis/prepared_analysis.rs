use super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct PreparedProgramAnalysis {
    pub(in crate::backend::direct_wasm) assigned_nonlocal_binding_results:
        Rc<HashMap<String, HashMap<String, Expression>>>,
    pub(in crate::backend::direct_wasm) shared: PreparedSharedProgramContext,
}

impl PreparedProgramAnalysis {
    pub(in crate::backend::direct_wasm) fn new(
        assigned_nonlocal_binding_results: HashMap<String, HashMap<String, Expression>>,
        user_function_metadata: HashMap<String, PreparedFunctionMetadata>,
        user_function_order: Vec<String>,
        eval_local_function_bindings: HashMap<String, HashMap<String, String>>,
        user_function_capture_bindings: HashMap<String, HashMap<String, String>>,
        global_binding_environment: GlobalBindingEnvironment,
        global_static_semantics: GlobalStaticSemanticsSnapshot,
    ) -> Self {
        Self {
            assigned_nonlocal_binding_results: Rc::new(assigned_nonlocal_binding_results),
            shared: PreparedSharedProgramContext {
                user_function_metadata: Rc::new(user_function_metadata),
                user_function_order: Rc::new(user_function_order),
                eval_local_function_bindings: Rc::new(eval_local_function_bindings),
                user_function_capture_bindings: Rc::new(user_function_capture_bindings),
                globals: PreparedGlobalProgramContext::new(
                    global_binding_environment,
                    global_static_semantics,
                ),
            },
        }
    }

    pub(in crate::backend::direct_wasm) fn assigned_nonlocal_binding_results(
        &self,
        function_name: &str,
    ) -> Option<&HashMap<String, Expression>> {
        self.assigned_nonlocal_binding_results.get(function_name)
    }

    pub(in crate::backend::direct_wasm) fn user_function_metadata(
        &self,
        function_name: &str,
    ) -> Option<&PreparedFunctionMetadata> {
        self.shared.user_function_metadata(function_name)
    }

    pub(in crate::backend::direct_wasm) fn user_function(
        &self,
        function_name: &str,
    ) -> Option<&UserFunction> {
        self.shared.user_function(function_name)
    }

    pub(in crate::backend::direct_wasm) fn user_function_declaration(
        &self,
        function_name: &str,
    ) -> Option<&FunctionDeclaration> {
        self.shared.user_function_declaration(function_name)
    }

    pub(in crate::backend::direct_wasm) fn ordered_user_functions(&self) -> Vec<UserFunction> {
        self.shared.ordered_user_functions()
    }

    pub(in crate::backend::direct_wasm) fn contains_user_function(&self, name: &str) -> bool {
        self.shared.contains_user_function(name)
    }

    pub(in crate::backend::direct_wasm) fn resolve_user_function_by_binding_name(
        &self,
        name: &str,
    ) -> Option<&UserFunction> {
        self.shared.resolve_user_function_by_binding_name(name)
    }

    pub(in crate::backend::direct_wasm) fn user_function_capture_bindings(
        &self,
        function_name: &str,
    ) -> Option<&HashMap<String, String>> {
        self.shared.user_function_capture_bindings(function_name)
    }

    pub(in crate::backend::direct_wasm) fn eval_local_function_bindings(
        &self,
        function_name: &str,
    ) -> Option<&HashMap<String, String>> {
        self.shared.eval_local_function_bindings(function_name)
    }

    pub(in crate::backend::direct_wasm) fn required_shared_global_binding_environment(
        &self,
    ) -> &SharedGlobalBindingEnvironment {
        self.shared.required_shared_global_binding_environment()
    }

    pub(in crate::backend::direct_wasm) fn required_global_static_semantics(
        &self,
    ) -> &GlobalStaticSemanticsSnapshot {
        self.shared.required_global_static_semantics()
    }

    pub(in crate::backend::direct_wasm) fn shared_program_context(
        &self,
    ) -> PreparedSharedProgramContext {
        self.shared.clone()
    }

    pub(in crate::backend::direct_wasm) fn assigned_nonlocal_binding_results_snapshot(
        &self,
    ) -> Rc<HashMap<String, HashMap<String, Expression>>> {
        self.assigned_nonlocal_binding_results.clone()
    }

    pub(in crate::backend::direct_wasm) fn function_compiler_inputs(
        &self,
    ) -> PreparedFunctionCompilerInputs {
        PreparedFunctionCompilerInputs {
            shared_program: self.shared_program_context(),
            assigned_nonlocal_binding_results: self.assigned_nonlocal_binding_results_snapshot(),
        }
    }
}
