use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_inline_user_function_summary_with_argument_locals(
        &mut self,
        user_function: &UserFunction,
        argument_locals: &[u32],
        argument_count: usize,
    ) -> DirectResult<bool> {
        let Some(summary) = user_function.inline_summary.as_ref() else {
            return Ok(false);
        };
        if !user_function.extra_argument_indices.is_empty()
            || user_function.has_parameter_defaults()
            || (inline_summary_mentions_call_frame_state(summary) && !user_function.lexical_this)
            || argument_locals.len() < argument_count
        {
            return Ok(false);
        }

        let mut argument_names = Vec::with_capacity(argument_count);
        for (index, argument_local) in argument_locals
            .iter()
            .copied()
            .take(argument_count)
            .enumerate()
        {
            let hidden_name = self.allocate_named_hidden_local(
                &format!("inline_arg_{index}"),
                StaticValueKind::Unknown,
            );
            let hidden_local = self
                .state
                .runtime
                .locals
                .get(&hidden_name)
                .copied()
                .expect("hidden inline argument local should exist");
            self.push_local_get(argument_local);
            self.push_local_set(hidden_local);
            argument_names.push(hidden_name);
        }

        let call_arguments = argument_names
            .into_iter()
            .map(Expression::Identifier)
            .map(CallArgument::Expression)
            .collect::<Vec<_>>();
        self.emit_inline_summary_with_call_arguments(user_function, summary, &call_arguments)?;
        Ok(true)
    }
}
