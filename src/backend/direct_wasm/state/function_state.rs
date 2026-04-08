use super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct FunctionParameterState {
    pub(in crate::backend::direct_wasm) parameter_names: Vec<String>,
    pub(in crate::backend::direct_wasm) parameter_defaults: Vec<Option<Expression>>,
    pub(in crate::backend::direct_wasm) parameter_initialized_locals: HashMap<String, u32>,
    pub(in crate::backend::direct_wasm) parameter_scope_arguments_local: Option<u32>,
    pub(in crate::backend::direct_wasm) in_parameter_default_initialization: bool,
    pub(in crate::backend::direct_wasm) param_count: u32,
    pub(in crate::backend::direct_wasm) visible_param_count: u32,
    pub(in crate::backend::direct_wasm) actual_argument_count_local: Option<u32>,
    pub(in crate::backend::direct_wasm) extra_argument_param_locals: HashMap<u32, u32>,
    pub(in crate::backend::direct_wasm) arguments_slots: HashMap<u32, ArgumentsSlot>,
    pub(in crate::backend::direct_wasm) mapped_arguments: bool,
    pub(in crate::backend::direct_wasm) local_arguments_bindings:
        HashMap<String, ArgumentsValueBinding>,
    pub(in crate::backend::direct_wasm) direct_arguments_aliases: HashSet<String>,
}

impl FunctionParameterState {
    pub(in crate::backend::direct_wasm) fn from_prepared_state(
        prepared: PreparedFunctionParameterState,
    ) -> Self {
        Self {
            parameter_names: prepared.parameter_names,
            parameter_defaults: prepared.parameter_defaults,
            parameter_initialized_locals: prepared.parameter_initialized_locals,
            parameter_scope_arguments_local: prepared.parameter_scope_arguments_local,
            in_parameter_default_initialization: false,
            param_count: prepared.param_count,
            visible_param_count: prepared.visible_param_count,
            actual_argument_count_local: prepared.actual_argument_count_local,
            extra_argument_param_locals: prepared.extra_argument_param_locals,
            arguments_slots: HashMap::new(),
            mapped_arguments: prepared.mapped_arguments,
            local_arguments_bindings: HashMap::new(),
            direct_arguments_aliases: HashSet::new(),
        }
    }

    pub(in crate::backend::direct_wasm) fn clear_local_binding_metadata(&mut self, name: &str) {
        self.local_arguments_bindings.remove(name);
    }

    pub(in crate::backend::direct_wasm) fn clear_isolated_indirect_eval_state(&mut self) {
        self.local_arguments_bindings.clear();
        self.direct_arguments_aliases.clear();
    }
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct FunctionCompilerState {
    pub(in crate::backend::direct_wasm) parameters: FunctionParameterState,
    pub(in crate::backend::direct_wasm) runtime: FunctionRuntimeState,
    pub(in crate::backend::direct_wasm) speculation: FunctionSpeculationState,
    pub(in crate::backend::direct_wasm) emission: FunctionEmissionState,
}
