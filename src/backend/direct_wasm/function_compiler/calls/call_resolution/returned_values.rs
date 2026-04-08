use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_user_function_from_callee_name(
        &self,
        name: &str,
    ) -> Option<&UserFunction> {
        let resolved_name = self
            .resolve_current_local_binding(name)
            .map(|(resolved_name, _)| resolved_name)
            .unwrap_or_else(|| name.to_string());
        if let Some(LocalFunctionBinding::User(function_name)) = self
            .state
            .speculation
            .static_semantics
            .local_function_binding(&resolved_name)
        {
            return self
                .backend
                .function_registry
                .catalog
                .user_function(&function_name);
        }
        self.resolve_user_function_by_binding_name(name)
    }

    pub(in crate::backend::direct_wasm) fn resolve_returned_member_value_from_expression(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<Expression> {
        let Expression::String(property_name) = property else {
            return None;
        };

        let (callee, arguments) = match object {
            Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
                (callee.as_ref(), arguments.as_slice())
            }
            _ => return None,
        };
        if let Some(object_binding) =
            self.resolve_returned_object_binding_from_call(callee, arguments)
            && let Some(value) = object_binding_lookup_value(
                &object_binding,
                &Expression::String(property_name.clone()),
            )
        {
            return Some(value.clone());
        }
        let LocalFunctionBinding::User(function_name) =
            self.resolve_function_binding_from_expression(callee)?
        else {
            return None;
        };
        let user_function = self.user_function(&function_name)?;
        let binding = user_function
            .returned_member_value_bindings
            .iter()
            .find(|binding| binding.property == *property_name)?;

        let mut value = self.substitute_user_function_argument_bindings(
            &binding.value,
            user_function,
            arguments,
        );
        if let Expression::Member { object, property } = callee
            && let Some(capture_slots) =
                self.resolve_member_function_capture_slots(object, property)
        {
            value = self.substitute_capture_slot_bindings(&value, &capture_slots);
        }

        Some(value)
    }

    pub(in crate::backend::direct_wasm) fn resolve_returned_object_binding_from_call(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Option<ObjectValueBinding> {
        let LocalFunctionBinding::User(function_name) =
            self.resolve_function_binding_from_expression(callee)?
        else {
            return None;
        };
        if let Some(snapshot) = self
            .state
            .speculation
            .static_semantics
            .last_bound_user_function_call
            .as_ref()
            .filter(|snapshot| snapshot.function_name == function_name)
            && let Some(result) = snapshot.result_expression.as_ref()
            && let Some(object_binding) = self.resolve_object_binding_from_expression(&result)
        {
            return Some(object_binding);
        }
        if let Some(object_binding) = self
            .resolve_static_returned_object_binding_from_user_function_call(
                &function_name,
                arguments,
            )
        {
            return Some(object_binding);
        }
        let user_function = self.user_function(&function_name)?;
        if user_function.returned_member_value_bindings.is_empty() {
            return None;
        }
        let capture_bindings = match callee {
            Expression::Member { object, property } => self
                .resolve_member_function_capture_slots(object, property)
                .unwrap_or_default(),
            _ => BTreeMap::new(),
        };
        let mut object_binding = empty_object_value_binding();
        for binding in &user_function.returned_member_value_bindings {
            let mut value = self.substitute_user_function_argument_bindings(
                &binding.value,
                user_function,
                arguments,
            );
            if !capture_bindings.is_empty() {
                value = self.substitute_capture_slot_bindings(&value, &capture_bindings);
            }
            object_binding_set_property(
                &mut object_binding,
                Expression::String(binding.property.clone()),
                value,
            );
        }
        Some(object_binding)
    }

    pub(in crate::backend::direct_wasm) fn resolve_returned_function_binding_from_call(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Option<LocalFunctionBinding> {
        let LocalFunctionBinding::User(function_name) =
            self.resolve_function_binding_from_expression(callee)?
        else {
            return None;
        };
        let user_function = self.user_function(&function_name)?;
        let function = self.resolve_registered_function_declaration(&function_name)?;
        let returned_expression = collect_returned_identifier_source_expression(&function.body)?;
        let substituted_expression = self.substitute_user_function_argument_bindings(
            &returned_expression,
            user_function,
            arguments,
        );
        self.resolve_function_binding_from_expression(&substituted_expression)
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_returned_object_binding_from_user_function_call(
        &self,
        function_name: &str,
        arguments: &[CallArgument],
    ) -> Option<ObjectValueBinding> {
        let user_function = self.user_function(function_name)?;
        let mut execution = self.prepare_static_user_function_execution(
            function_name,
            user_function,
            arguments,
            &Expression::Undefined,
            None,
            HashMap::new(),
            |statement| statement,
        )?;
        let return_value = self.execute_static_statements_with_state(
            &execution.substituted_body,
            &mut execution.environment,
        )??;
        self.resolve_object_binding_from_expression_with_state(
            &return_value,
            &mut execution.environment,
        )
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_returned_descriptor_binding_from_user_function_call(
        &self,
        function_name: &str,
        arguments: &[CallArgument],
    ) -> Option<PropertyDescriptorBinding> {
        let user_function = self.user_function(function_name)?;
        let mut execution = self.prepare_static_user_function_execution(
            function_name,
            user_function,
            arguments,
            &Expression::Undefined,
            None,
            HashMap::new(),
            |statement| statement,
        )?;
        let return_value = self.execute_static_statements_with_state(
            &execution.substituted_body,
            &mut execution.environment,
        )??;
        self.resolve_descriptor_binding_from_expression_with_state(
            &return_value,
            &execution.environment,
        )
    }
}
