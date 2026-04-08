use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn apply_current_arguments_effect(
        &mut self,
        property_name: &str,
        effect: ArgumentsPropertyEffect,
    ) {
        match property_name {
            "callee" => {
                if self.state.speculation.execution_context.strict_mode {
                    return;
                }
                match effect {
                    ArgumentsPropertyEffect::Assign(value) => {
                        self.state
                            .speculation
                            .execution_context
                            .current_arguments_callee_present = true;
                        self.state
                            .speculation
                            .execution_context
                            .current_arguments_callee_override = Some(value);
                    }
                    ArgumentsPropertyEffect::Delete => {
                        self.state
                            .speculation
                            .execution_context
                            .current_arguments_callee_present = false;
                        self.state
                            .speculation
                            .execution_context
                            .current_arguments_callee_override = None;
                    }
                }
            }
            "length" => match effect {
                ArgumentsPropertyEffect::Assign(value) => {
                    self.state
                        .speculation
                        .execution_context
                        .current_arguments_length_present = true;
                    self.state
                        .speculation
                        .execution_context
                        .current_arguments_length_override = Some(value);
                }
                ArgumentsPropertyEffect::Delete => {
                    self.state
                        .speculation
                        .execution_context
                        .current_arguments_length_present = false;
                    self.state
                        .speculation
                        .execution_context
                        .current_arguments_length_override = None;
                }
            },
            _ => {}
        }
    }

    pub(in crate::backend::direct_wasm) fn update_named_arguments_binding_effect(
        &mut self,
        object: &Expression,
        property_name: &str,
        effect: ArgumentsPropertyEffect,
    ) -> bool {
        let Expression::Identifier(name) = object else {
            return false;
        };
        if let Some(binding) = self.state.parameters.local_arguments_bindings.get_mut(name) {
            binding.apply_named_effect(property_name, effect.clone());
            return true;
        }
        if self
            .backend
            .apply_global_arguments_binding_named_effect(name, property_name, effect)
        {
            return true;
        }
        false
    }
}
