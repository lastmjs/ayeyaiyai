use super::*;

impl<'a> FunctionCompiler<'a> {
    fn seed_static_user_function_capture_bindings_with_sources(
        &self,
        function_name: &str,
        capture_source_bindings: Option<&HashMap<String, Expression>>,
        local_bindings: &mut HashMap<String, Expression>,
    ) {
        let snapshot_updated_bindings = self
            .state
            .speculation
            .static_semantics
            .last_bound_user_function_call
            .as_ref()
            .map(|snapshot| &snapshot.updated_bindings);
        if let Some(capture_bindings) = self.user_function_capture_bindings(function_name) {
            for (source_name, hidden_name) in capture_bindings {
                local_bindings.insert(
                    source_name.clone(),
                    capture_source_bindings
                        .and_then(|bindings| bindings.get(&source_name).cloned())
                        .or_else(|| self.global_value_binding(&hidden_name).cloned())
                        .or_else(|| {
                            snapshot_updated_bindings
                                .and_then(|bindings| bindings.get(&source_name).cloned())
                        })
                        .unwrap_or_else(|| Expression::Identifier(hidden_name.clone())),
                );
            }
        }
    }

    fn expand_static_user_function_call_arguments(
        &self,
        arguments: &[CallArgument],
    ) -> Vec<CallArgument> {
        self.expand_call_arguments(arguments)
            .into_iter()
            .map(CallArgument::Expression)
            .collect()
    }

    fn static_user_function_arguments_binding(arguments: &[CallArgument]) -> Expression {
        Expression::Array(
            arguments
                .iter()
                .map(|argument| match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        ArrayElement::Expression(expression.clone())
                    }
                })
                .collect(),
        )
    }

    pub(in crate::backend::direct_wasm) fn prepare_static_user_function_execution(
        &self,
        function_name: &str,
        user_function: &UserFunction,
        arguments: &[CallArgument],
        this_binding: &Expression,
        capture_source_bindings: Option<&HashMap<String, Expression>>,
        extra_local_bindings: HashMap<String, Expression>,
        mut transform_statement: impl FnMut(Statement) -> Statement,
    ) -> Option<PreparedStaticUserFunctionExecution> {
        let function = self.resolve_registered_function_declaration(function_name)?;
        let call_arguments = self.expand_static_user_function_call_arguments(arguments);
        let arguments_binding = Self::static_user_function_arguments_binding(&call_arguments);
        let substituted_body = function
            .body
            .iter()
            .map(|statement| {
                transform_statement(self.substitute_user_function_statement_call_frame_bindings(
                    statement,
                    user_function,
                    &call_arguments,
                    this_binding,
                    &arguments_binding,
                ))
            })
            .collect::<Vec<_>>();
        let mut local_bindings = extra_local_bindings;
        self.seed_static_user_function_capture_bindings_with_sources(
            function_name,
            capture_source_bindings,
            &mut local_bindings,
        );
        let environment =
            self.snapshot_static_resolution_environment_with_local_bindings(local_bindings);
        Some(PreparedStaticUserFunctionExecution {
            substituted_body,
            environment,
        })
    }
}
