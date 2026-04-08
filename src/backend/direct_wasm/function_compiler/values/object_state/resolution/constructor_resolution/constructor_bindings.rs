use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_user_constructor_object_binding_for_function(
        &self,
        user_function: &UserFunction,
        arguments: &[CallArgument],
        capture_source_bindings: Option<&HashMap<String, Expression>>,
    ) -> Option<ObjectValueBinding> {
        self.resolve_user_constructor_object_binding_for_function_with_this_binding(
            user_function,
            arguments,
            capture_source_bindings,
            empty_object_value_binding(),
        )
    }

    pub(in crate::backend::direct_wasm) fn resolve_user_constructor_object_binding_for_function_with_this_binding(
        &self,
        user_function: &UserFunction,
        arguments: &[CallArgument],
        capture_source_bindings: Option<&HashMap<String, Expression>>,
        this_object_binding: ObjectValueBinding,
    ) -> Option<ObjectValueBinding> {
        if !user_function.is_constructible() {
            return None;
        }

        let this_name = Self::STATIC_NEW_THIS_BINDING.to_string();
        let this_binding = Expression::Identifier(this_name.clone());
        let mut extra_local_bindings = HashMap::new();
        extra_local_bindings.insert(
            Self::STATIC_NEW_THIS_INITIALIZED_BINDING.to_string(),
            Expression::Bool(false),
        );
        let mut execution = self.prepare_static_user_function_execution(
            &user_function.name,
            user_function,
            arguments,
            &this_binding,
            capture_source_bindings,
            extra_local_bindings,
            |statement| Self::substitute_static_constructor_new_target_statement(&statement),
        )?;

        if !self
            .collect_user_function_assigned_nonlocal_bindings(user_function)
            .is_empty()
            && execution
                .substituted_body
                .iter()
                .any(Self::statement_contains_static_constructor_snapshot_call)
        {
            return None;
        }

        execution
            .environment
            .set_local_object_binding(this_name.clone(), this_object_binding.clone());

        let return_value = self.execute_static_statements_with_state(
            &execution.substituted_body,
            &mut execution.environment,
        );
        if return_value.is_none()
            && self.user_function_is_derived_constructor(user_function)
            && execution
                .environment
                .object_binding(&this_name)
                .is_some_and(|binding| binding != &this_object_binding)
        {
            return execution.environment.object_binding(&this_name).cloned();
        }
        let return_value = return_value?;
        if let Some(return_value) = return_value
            && let Some(returned_object) = self.resolve_object_binding_from_expression_with_state(
                &return_value,
                &mut execution.environment,
            )
        {
            return Some(returned_object);
        }
        execution.environment.object_binding(&this_name).cloned()
    }

    pub(in crate::backend::direct_wasm) fn resolve_user_constructor_object_binding_from_new(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Option<ObjectValueBinding> {
        let LocalFunctionBinding::User(function_name) =
            self.resolve_function_binding_from_expression(callee)?
        else {
            return None;
        };
        let user_function = self.user_function(&function_name)?;
        let capture_source_bindings =
            self.resolve_constructor_capture_source_bindings_from_expression(callee);
        self.resolve_user_constructor_object_binding_for_function(
            user_function,
            arguments,
            capture_source_bindings.as_ref(),
        )
    }

    pub(in crate::backend::direct_wasm) fn resolve_constructor_capture_source_bindings_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<HashMap<String, Expression>> {
        let resolved = self
            .resolve_bound_alias_expression(expression)
            .or_else(|| match expression {
                Expression::Identifier(name) => self
                    .state
                    .speculation
                    .static_semantics
                    .local_value_binding(name)
                    .cloned()
                    .or_else(|| {
                        self.backend
                            .global_semantics
                            .values
                            .value_bindings
                            .get(name)
                            .cloned()
                    }),
                _ => None,
            })
            .unwrap_or_else(|| expression.clone());
        let mut call_arguments = None;
        let callee = match &resolved {
            Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
                call_arguments = Some(arguments.as_slice());
                callee.as_ref()
            }
            _ => &resolved,
        };
        let LocalFunctionBinding::User(function_name) =
            self.resolve_function_binding_from_expression(callee)?
        else {
            return None;
        };
        let user_function = self.user_function(&function_name)?;
        if call_arguments.is_none() {
            let capture_bindings = self
                .backend
                .function_registry
                .analysis
                .user_function_capture_bindings
                .get(&function_name)?;
            let mut bindings = HashMap::new();
            for source_name in capture_bindings.keys() {
                if !self.user_function_capture_source_is_locally_bound(source_name) {
                    return None;
                }
                let source_expression = Expression::Identifier(source_name.clone());
                let resolved_source = self
                    .resolve_bound_alias_expression(&source_expression)
                    .filter(|resolved| !static_expression_matches(resolved, &source_expression))
                    .unwrap_or(source_expression);
                bindings.insert(source_name.clone(), resolved_source);
            }
            return Some(bindings);
        }
        let arguments = call_arguments.expect("filtered above");
        let mut execution = self.prepare_static_user_function_execution(
            &function_name,
            user_function,
            arguments,
            &Expression::Undefined,
            None,
            HashMap::new(),
            |statement| statement,
        )?;
        self.execute_static_statements_with_state(
            &execution.substituted_body,
            &mut execution.environment,
        )?;
        Some(execution.environment.into_local_bindings())
    }
}
