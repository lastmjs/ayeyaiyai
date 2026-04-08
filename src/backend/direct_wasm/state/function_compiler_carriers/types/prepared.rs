use super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct PreparedFunctionExecutionContext {
    pub(in crate::backend::direct_wasm) strict_mode: bool,
    pub(in crate::backend::direct_wasm) current_user_function_name: Option<String>,
    pub(in crate::backend::direct_wasm) current_user_function: Option<UserFunction>,
    pub(in crate::backend::direct_wasm) current_function_declaration: Option<FunctionDeclaration>,
    pub(in crate::backend::direct_wasm) current_arguments_callee_present: bool,
    pub(in crate::backend::direct_wasm) current_arguments_length_present: bool,
    pub(in crate::backend::direct_wasm) top_level_function: bool,
    pub(in crate::backend::direct_wasm) derived_constructor: bool,
    pub(in crate::backend::direct_wasm) self_binding_local: Option<u32>,
    pub(in crate::backend::direct_wasm) self_binding_runtime_value: Option<i32>,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct PreparedFunctionParameterState {
    pub(in crate::backend::direct_wasm) parameter_names: Vec<String>,
    pub(in crate::backend::direct_wasm) parameter_defaults: Vec<Option<Expression>>,
    pub(in crate::backend::direct_wasm) parameter_initialized_locals: HashMap<String, u32>,
    pub(in crate::backend::direct_wasm) parameter_scope_arguments_local: Option<u32>,
    pub(in crate::backend::direct_wasm) param_count: u32,
    pub(in crate::backend::direct_wasm) visible_param_count: u32,
    pub(in crate::backend::direct_wasm) actual_argument_count_local: Option<u32>,
    pub(in crate::backend::direct_wasm) extra_argument_param_locals: HashMap<u32, u32>,
    pub(in crate::backend::direct_wasm) mapped_arguments: bool,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct PreparedFunctionRuntimeState {
    pub(in crate::backend::direct_wasm) locals: HashMap<String, u32>,
    pub(in crate::backend::direct_wasm) throw_tag_local: u32,
    pub(in crate::backend::direct_wasm) throw_value_local: u32,
    pub(in crate::backend::direct_wasm) next_local_index: u32,
    pub(in crate::backend::direct_wasm) allow_return: bool,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct PreparedLocalStaticBindings {
    pub(in crate::backend::direct_wasm) local_kinds: HashMap<String, StaticValueKind>,
    pub(in crate::backend::direct_wasm) local_value_bindings: HashMap<String, Expression>,
    pub(in crate::backend::direct_wasm) local_function_bindings:
        HashMap<String, LocalFunctionBinding>,
    pub(in crate::backend::direct_wasm) local_array_bindings: HashMap<String, ArrayValueBinding>,
    pub(in crate::backend::direct_wasm) local_object_bindings: HashMap<String, ObjectValueBinding>,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct PreparedFunctionEntryState {
    pub(in crate::backend::direct_wasm) parameter_state: PreparedFunctionParameterState,
    pub(in crate::backend::direct_wasm) runtime: PreparedFunctionRuntimeState,
    pub(in crate::backend::direct_wasm) static_bindings: PreparedLocalStaticBindings,
    pub(in crate::backend::direct_wasm) execution_context: PreparedFunctionExecutionContext,
}
