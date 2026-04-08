use super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct FunctionExecutionContextState {
    pub(in crate::backend::direct_wasm) strict_mode: bool,
    pub(in crate::backend::direct_wasm) current_user_function_name: Option<String>,
    pub(in crate::backend::direct_wasm) current_user_function: Option<UserFunction>,
    pub(in crate::backend::direct_wasm) current_function_declaration: Option<FunctionDeclaration>,
    pub(in crate::backend::direct_wasm) current_arguments_callee_present: bool,
    pub(in crate::backend::direct_wasm) current_arguments_callee_override: Option<Expression>,
    pub(in crate::backend::direct_wasm) current_arguments_length_present: bool,
    pub(in crate::backend::direct_wasm) current_arguments_length_override: Option<Expression>,
    pub(in crate::backend::direct_wasm) top_level_function: bool,
    pub(in crate::backend::direct_wasm) derived_constructor: bool,
    pub(in crate::backend::direct_wasm) self_binding_local: Option<u32>,
    pub(in crate::backend::direct_wasm) self_binding_runtime_value: Option<i32>,
    pub(in crate::backend::direct_wasm) isolated_indirect_eval: bool,
}

impl FunctionExecutionContextState {
    pub(in crate::backend::direct_wasm) fn from_prepared_context(
        prepared: PreparedFunctionExecutionContext,
    ) -> Self {
        Self {
            strict_mode: prepared.strict_mode,
            current_user_function_name: prepared.current_user_function_name,
            current_user_function: prepared.current_user_function,
            current_function_declaration: prepared.current_function_declaration,
            current_arguments_callee_present: prepared.current_arguments_callee_present,
            current_arguments_callee_override: None,
            current_arguments_length_present: prepared.current_arguments_length_present,
            current_arguments_length_override: None,
            top_level_function: prepared.top_level_function,
            derived_constructor: prepared.derived_constructor,
            self_binding_local: prepared.self_binding_local,
            self_binding_runtime_value: prepared.self_binding_runtime_value,
            isolated_indirect_eval: false,
        }
    }

    pub(in crate::backend::direct_wasm) fn reset_isolated_indirect_eval_entry(&mut self) {
        self.current_user_function_name = None;
        self.current_arguments_callee_present = false;
        self.current_arguments_callee_override = None;
        self.current_arguments_length_present = false;
        self.current_arguments_length_override = None;
        self.top_level_function = true;
        self.strict_mode = false;
        self.isolated_indirect_eval = true;
    }
}
