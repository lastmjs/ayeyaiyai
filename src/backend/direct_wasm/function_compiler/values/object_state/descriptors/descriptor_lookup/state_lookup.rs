use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_descriptor_binding_from_expression_with_state(
        &self,
        expression: &Expression,
        environment: &StaticResolutionEnvironment,
    ) -> Option<PropertyDescriptorBinding> {
        if let Expression::Identifier(name) = expression {
            let resolved =
                self.resolve_bound_alias_expression_with_state(expression, environment)?;
            if let Expression::Identifier(resolved_name) = resolved {
                let resolved_name = self
                    .resolve_current_local_binding(&resolved_name)
                    .map(|(resolved_name, _)| resolved_name)
                    .unwrap_or(resolved_name);
                return environment
                    .descriptor_binding(&resolved_name)
                    .cloned()
                    .or_else(|| {
                        (resolved_name != *name)
                            .then(|| environment.descriptor_binding(name).cloned())
                            .flatten()
                    });
            }
        }
        let Expression::Call { callee, arguments } = expression else {
            return None;
        };
        let resolved_callee =
            self.resolve_bound_alias_expression_with_state(callee, environment)?;
        let resolved_arguments = arguments
            .iter()
            .map(|argument| match argument {
                CallArgument::Expression(argument) => self
                    .materialize_static_expression_with_state(argument, environment)
                    .map(CallArgument::Expression),
                CallArgument::Spread(argument) => self
                    .materialize_static_expression_with_state(argument, environment)
                    .map(CallArgument::Spread),
            })
            .collect::<Option<Vec<_>>>()?;
        self.resolve_descriptor_binding_from_expression(&Expression::Call {
            callee: Box::new(resolved_callee),
            arguments: resolved_arguments,
        })
    }
}
