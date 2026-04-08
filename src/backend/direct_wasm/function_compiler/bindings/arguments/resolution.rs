use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_arguments_callee_strictness(
        &self,
        expression: &Expression,
    ) -> Option<bool> {
        let Expression::Member { object, property } = expression else {
            return None;
        };
        if !matches!(property.as_ref(), Expression::String(property_name) if property_name == "callee")
        {
            return None;
        }
        if self.is_direct_arguments_object(object) {
            return Some(self.state.speculation.execution_context.strict_mode);
        }
        self.resolve_arguments_binding_from_expression(object)
            .map(|binding| binding.strict)
    }

    pub(in crate::backend::direct_wasm) fn resolve_arguments_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<ArgumentsValueBinding> {
        match expression {
            Expression::Identifier(name) => {
                let resolved_name = scoped_binding_source_name(name).unwrap_or(name);
                self.state
                    .parameters
                    .local_arguments_bindings
                    .get(name)
                    .cloned()
                    .or_else(|| {
                        self.state
                            .parameters
                            .local_arguments_bindings
                            .get(resolved_name)
                            .cloned()
                    })
                    .or_else(|| self.backend.global_arguments_binding(name).cloned())
                    .or_else(|| {
                        self.backend
                            .global_arguments_binding(resolved_name)
                            .cloned()
                    })
            }
            Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                let user_function = self.resolve_user_function_from_callee_name(name)?;
                if !user_function.returns_arguments_object {
                    return None;
                }
                Some(ArgumentsValueBinding::for_user_function(
                    user_function,
                    self.expand_call_arguments(arguments),
                ))
            }
            _ => None,
        }
    }
}
