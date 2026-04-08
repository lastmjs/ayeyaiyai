use super::super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct UserFunction {
    pub(in crate::backend::direct_wasm) name: String,
    pub(in crate::backend::direct_wasm) kind: FunctionKind,
    pub(in crate::backend::direct_wasm) params: Vec<String>,
    pub(in crate::backend::direct_wasm) scope_bindings: HashSet<String>,
    pub(in crate::backend::direct_wasm) parameter_defaults: Vec<Option<Expression>>,
    pub(in crate::backend::direct_wasm) body_declares_arguments_binding: bool,
    pub(in crate::backend::direct_wasm) length: u32,
    pub(in crate::backend::direct_wasm) extra_argument_indices: Vec<u32>,
    pub(in crate::backend::direct_wasm) enumerated_keys_param_index: Option<usize>,
    pub(in crate::backend::direct_wasm) returns_arguments_object: bool,
    pub(in crate::backend::direct_wasm) returned_arguments_effects: ReturnedArgumentsEffects,
    pub(in crate::backend::direct_wasm) returned_member_function_bindings:
        Vec<ReturnedMemberFunctionBinding>,
    pub(in crate::backend::direct_wasm) returned_member_value_bindings:
        Vec<ReturnedMemberValueBinding>,
    pub(in crate::backend::direct_wasm) inline_summary: Option<InlineFunctionSummary>,
    pub(in crate::backend::direct_wasm) home_object_binding: Option<String>,
    pub(in crate::backend::direct_wasm) strict: bool,
    pub(in crate::backend::direct_wasm) lexical_this: bool,
    pub(in crate::backend::direct_wasm) function_index: u32,
    pub(in crate::backend::direct_wasm) type_index: u32,
}

impl UserFunction {
    pub(in crate::backend::direct_wasm) fn is_async(&self) -> bool {
        self.kind.is_async()
    }

    pub(in crate::backend::direct_wasm) fn is_arrow(&self) -> bool {
        self.lexical_this
    }

    pub(in crate::backend::direct_wasm) fn is_generator(&self) -> bool {
        self.kind.is_generator()
    }

    pub(in crate::backend::direct_wasm) fn is_constructible(&self) -> bool {
        matches!(self.kind, FunctionKind::Ordinary) && !self.lexical_this
    }

    pub(in crate::backend::direct_wasm) fn visible_param_count(&self) -> u32 {
        self.params.len() as u32
    }

    pub(in crate::backend::direct_wasm) fn has_parameter_defaults(&self) -> bool {
        self.parameter_defaults.iter().any(Option::is_some)
    }

    pub(in crate::backend::direct_wasm) fn has_lowered_pattern_parameters(&self) -> bool {
        self.params
            .iter()
            .any(|name| name.starts_with("__ayy_param_") || name.starts_with("__ayy_rest_"))
    }

    pub(in crate::backend::direct_wasm) fn wasm_param_count(&self) -> u32 {
        self.visible_param_count() + 1 + self.extra_argument_indices.len() as u32
    }

    pub(in crate::backend::direct_wasm) fn actual_argument_count_param(&self) -> u32 {
        self.visible_param_count()
    }

    pub(in crate::backend::direct_wasm) fn extra_argument_param(&self, index: u32) -> Option<u32> {
        let position = self
            .extra_argument_indices
            .iter()
            .position(|candidate| *candidate == index)? as u32;
        Some(self.visible_param_count() + 1 + position)
    }
}
