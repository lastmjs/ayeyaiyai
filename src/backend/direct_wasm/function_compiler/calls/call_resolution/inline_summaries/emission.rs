use super::*;

#[path = "emission/fallback_path.rs"]
mod fallback_path;
#[path = "emission/state_setup.rs"]
mod state_setup;
#[path = "emission/summary_path.rs"]
mod summary_path;

struct InlineSummaryEmissionState {
    prepared_capture_bindings: Vec<PreparedCaptureBinding>,
    assigned_nonlocal_bindings: HashSet<String>,
    call_effect_nonlocal_bindings: HashSet<String>,
    assigned_nonlocal_binding_results: Option<HashMap<String, Expression>>,
    additional_call_effect_nonlocal_bindings: HashSet<String>,
    updated_nonlocal_bindings: HashSet<String>,
    updated_bindings: Option<HashMap<String, Expression>>,
    arguments_binding: Expression,
    call_arguments: Vec<CallArgument>,
    inline_parameter_scope_names: Vec<String>,
}

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_inline_user_function_summary_with_explicit_call_frame(
        &mut self,
        user_function: &UserFunction,
        arguments: &[Expression],
        this_binding: &Expression,
        result_local: u32,
    ) -> DirectResult<bool> {
        if user_function.has_lowered_pattern_parameters()
            || !self
                .user_function_parameter_iterator_consumption_indices(user_function)
                .is_empty()
        {
            return Ok(false);
        }
        let mut state =
            self.prepare_inline_summary_emission_state(user_function, arguments, this_binding)?;
        if self.try_emit_inline_summary_fast_path(
            user_function,
            arguments,
            &state,
            this_binding,
            result_local,
        )? {
            self.finalize_inline_summary_emission_state(user_function, arguments, &mut state)?;
            return Ok(true);
        }
        let emitted = self.try_emit_inline_summary_fallback_path(
            user_function,
            &state,
            this_binding,
            result_local,
        )?;
        if !emitted {
            self.abort_inline_summary_emission_state(&state);
            return Ok(false);
        }
        self.finalize_inline_summary_emission_state(user_function, arguments, &mut state)?;
        Ok(true)
    }
}
