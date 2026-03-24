use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn expression_uses_runtime_dynamic_binding(
        &self,
        expression: &Expression,
    ) -> bool {
        if let Expression::Identifier(name) = expression
            && self.runtime_dynamic_bindings.contains(name)
        {
            return true;
        }
        self.resolve_bound_alias_expression(expression)
            .is_some_and(|resolved| {
                matches!(resolved, Expression::Identifier(name)
                    if self.runtime_dynamic_bindings.contains(&name))
            })
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_object_prototype_expression(
        &self,
        expression: &Expression,
    ) -> Option<Expression> {
        if let Some(resolved) = self
            .resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
        {
            return self.resolve_static_object_prototype_expression(&resolved);
        }

        if self.expression_is_known_array_value(expression) {
            return Some(Self::prototype_member_expression("Array"));
        }
        if self.expression_is_known_promise_instance_for_instanceof(expression) {
            return Some(Self::prototype_member_expression("Promise"));
        }
        if let Expression::Identifier(name) = expression {
            if let Some(prototype) = Self::builtin_constructor_object_prototype_expression(name) {
                return Some(prototype);
            }
            if self
                .resolve_function_binding_from_expression(expression)
                .is_some()
            {
                return Some(Self::prototype_member_expression("Function"));
            }
        }

        match expression {
            Expression::Identifier(name) => {
                if let Some(value) = self
                    .local_value_bindings
                    .get(name)
                    .or_else(|| self.module.global_value_bindings.get(name))
                    .filter(
                        |value| !matches!(value, Expression::Identifier(alias) if alias == name),
                    )
                    && let Some(prototype) = self.resolve_static_object_prototype_expression(value)
                {
                    return Some(prototype);
                }
                if let Some(prototype) = self.module.global_object_prototype_bindings.get(name) {
                    return Some(prototype.clone());
                }
                if let Some(prototype) = Self::builtin_constructor_object_prototype_expression(name)
                {
                    return Some(prototype);
                }
            }
            Expression::Object(_) => {
                return Some(
                    object_literal_prototype_expression(expression)
                        .unwrap_or_else(|| Self::prototype_member_expression("Object")),
                );
            }
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "prototype") =>
            {
                let Expression::Identifier(name) = object.as_ref() else {
                    return None;
                };
                if let Some(prototype) = Self::builtin_prototype_object_prototype_expression(name) {
                    return Some(prototype);
                }
                if self
                    .resolve_function_binding_from_expression(object)
                    .is_some()
                    || matches!(infer_call_result_kind(name), Some(_))
                {
                    return Some(Self::prototype_member_expression("Object"));
                }
            }
            Expression::New { callee, .. } => {
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                return Some(Self::prototype_member_expression(name));
            }
            Expression::Call { callee, .. } => {
                if matches!(
                    callee.as_ref(),
                    Expression::Member { object, property }
                        if matches!(object.as_ref(), Expression::Identifier(name) if name == "Promise")
                            && matches!(property.as_ref(), Expression::String(name) if name == "resolve")
                ) {
                    return Some(Self::prototype_member_expression("Promise"));
                }
                if self
                    .resolve_user_function_from_expression(callee.as_ref())
                    .is_some_and(|user_function| user_function.is_async())
                {
                    return Some(Self::prototype_member_expression("Promise"));
                }
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                if native_error_runtime_value(name).is_some() {
                    return Some(Self::prototype_member_expression(name));
                }
            }
            _ => {}
        }

        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.resolve_static_object_prototype_expression(&materialized);
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_weakref_target_expression(
        &self,
        expression: &Expression,
    ) -> Option<Expression> {
        if let Some(resolved) = self
            .resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
        {
            return self.resolve_static_weakref_target_expression(&resolved);
        }
        let Expression::New { callee, arguments } = expression else {
            let materialized = self.materialize_static_expression(expression);
            if !static_expression_matches(&materialized, expression) {
                return self.resolve_static_weakref_target_expression(&materialized);
            }
            return None;
        };
        if !matches!(callee.as_ref(), Expression::Identifier(name) if name == "WeakRef") {
            return None;
        }
        match arguments.first() {
            Some(CallArgument::Expression(target)) | Some(CallArgument::Spread(target)) => {
                Some(target.clone())
            }
            None => Some(Expression::Undefined),
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_user_constructor_object_binding_for_function(
        &self,
        user_function: &UserFunction,
        arguments: &[CallArgument],
    ) -> Option<ObjectValueBinding> {
        if !user_function.is_constructible() {
            return None;
        }

        let call_arguments = self
            .expand_call_arguments(arguments)
            .into_iter()
            .map(CallArgument::Expression)
            .collect::<Vec<_>>();
        let arguments_binding = Expression::Array(
            call_arguments
                .iter()
                .map(|argument| match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        ArrayElement::Expression(expression.clone())
                    }
                })
                .collect(),
        );
        let this_name = "__ayy_static_new_this".to_string();
        let this_binding = Expression::Identifier(this_name.clone());
        let substituted_body = self
            .resolve_registered_function_declaration(&user_function.name)?
            .body
            .iter()
            .map(|statement| {
                self.substitute_user_function_statement_call_frame_bindings(
                    statement,
                    user_function,
                    &call_arguments,
                    &this_binding,
                    &arguments_binding,
                )
            })
            .collect::<Vec<_>>();

        let mut local_bindings = HashMap::new();
        let mut value_bindings = self.module.global_value_bindings.clone();
        value_bindings.extend(self.local_value_bindings.clone());
        let mut object_bindings = self.module.global_object_bindings.clone();
        object_bindings.extend(self.local_object_bindings.clone());
        object_bindings.insert(this_name.clone(), empty_object_value_binding());

        let return_value = self.execute_static_statements_with_state(
            &substituted_body,
            &mut local_bindings,
            &mut value_bindings,
            &mut object_bindings,
        )?;
        if let Some(return_value) = return_value
            && let Some(returned_object) = self.resolve_object_binding_from_expression_with_state(
                &return_value,
                &local_bindings,
                &mut value_bindings,
                &mut object_bindings,
            )
        {
            return Some(returned_object);
        }
        object_bindings.get(&this_name).cloned()
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
        let user_function = self.module.user_function_map.get(&function_name)?;
        self.resolve_user_constructor_object_binding_for_function(user_function, arguments)
    }

    pub(in crate::backend::direct_wasm) fn resolve_object_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<ObjectValueBinding> {
        let binding = match expression {
            Expression::Identifier(name) => {
                if name == "$262" {
                    let mut host_object_binding = empty_object_value_binding();
                    object_binding_set_property(
                        &mut host_object_binding,
                        Expression::String("createRealm".to_string()),
                        Expression::Identifier(TEST262_CREATE_REALM_BUILTIN.to_string()),
                    );
                    return Some(host_object_binding);
                }
                if let Some(realm_id) = parse_test262_realm_identifier(name) {
                    return self.module.test262_realm_object_binding(realm_id);
                }
                if let Some(realm_id) = parse_test262_realm_global_identifier(name) {
                    return self
                        .module
                        .test262_realms
                        .get(&realm_id)
                        .map(|realm| realm.global_object_binding.clone());
                }
                self.local_object_bindings
                    .get(name)
                    .cloned()
                    .or_else(|| {
                        self.local_descriptor_bindings.get(name).map(|descriptor| {
                            let mut object_binding = empty_object_value_binding();
                            object_binding_set_property(
                                &mut object_binding,
                                Expression::String("configurable".to_string()),
                                Expression::Bool(descriptor.configurable),
                            );
                            object_binding_set_property(
                                &mut object_binding,
                                Expression::String("enumerable".to_string()),
                                Expression::Bool(descriptor.enumerable),
                            );
                            if let Some(value) = descriptor.value.clone() {
                                object_binding_set_property(
                                    &mut object_binding,
                                    Expression::String("value".to_string()),
                                    value,
                                );
                            }
                            if let Some(writable) = descriptor.writable {
                                object_binding_set_property(
                                    &mut object_binding,
                                    Expression::String("writable".to_string()),
                                    Expression::Bool(writable),
                                );
                            }
                            if descriptor.has_get {
                                object_binding_set_property(
                                    &mut object_binding,
                                    Expression::String("get".to_string()),
                                    Expression::Undefined,
                                );
                            }
                            if descriptor.has_set {
                                object_binding_set_property(
                                    &mut object_binding,
                                    Expression::String("set".to_string()),
                                    Expression::Undefined,
                                );
                            }
                            object_binding
                        })
                    })
                    .or_else(|| {
                        let hidden_name = self.resolve_user_function_capture_hidden_name(name)?;
                        self.module
                            .global_object_bindings
                            .get(&hidden_name)
                            .cloned()
                    })
                    .or_else(|| self.module.global_object_bindings.get(name).cloned())
                    .or_else(|| {
                        let proxy = self
                            .local_proxy_bindings
                            .get(name)
                            .cloned()
                            .or_else(|| self.module.global_proxy_bindings.get(name).cloned())?;
                        self.resolve_object_binding_from_expression(&proxy.target)
                    })
                    .or_else(|| {
                        let resolved = self.resolve_bound_alias_expression(expression)?;
                        (!static_expression_matches(&resolved, expression))
                            .then(|| self.resolve_object_binding_from_expression(&resolved))
                            .flatten()
                    })
            }
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "global") =>
            {
                let realm_id = self.resolve_test262_realm_id_from_expression(object)?;
                self.module
                    .test262_realms
                    .get(&realm_id)
                    .map(|realm| realm.global_object_binding.clone())
            }
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "prototype") =>
            {
                let Expression::Identifier(name) = object.as_ref() else {
                    return None;
                };
                self.resolve_function_prototype_object_binding(name)
            }
            Expression::GetIterator(iterated) => {
                if let Some(object_binding) = self.resolve_object_binding_from_expression(iterated)
                {
                    let has_next_method = object_binding_lookup_value(
                        &object_binding,
                        &Expression::String("next".to_string()),
                    )
                    .and_then(|value| self.resolve_function_binding_from_expression(value))
                    .is_some();
                    if has_next_method || self.resolve_iterator_source_kind(iterated).is_some() {
                        return Some(object_binding);
                    }
                }
                let iterator_callee = Expression::Member {
                    object: Box::new((**iterated).clone()),
                    property: Box::new(
                        self.materialize_static_expression(&symbol_iterator_expression()),
                    ),
                };
                self.resolve_object_binding_from_expression(&Expression::Call {
                    callee: Box::new(iterator_callee),
                    arguments: Vec::new(),
                })
            }
            Expression::Call { callee, .. }
                if matches!(
                    callee.as_ref(),
                    Expression::Member { object, property }
                        if matches!(object.as_ref(), Expression::Identifier(name) if name == "Object")
                            && matches!(property.as_ref(), Expression::String(name) if name == "create")
                ) =>
            {
                Some(ObjectValueBinding {
                    string_properties: Vec::new(),
                    symbol_properties: Vec::new(),
                    non_enumerable_string_properties: Vec::new(),
                })
            }
            Expression::Call { callee, arguments }
                if arguments.is_empty()
                    && matches!(
                        callee.as_ref(),
                        Expression::Member { property, .. }
                            if is_symbol_iterator_expression(property)
                    ) =>
            {
                let Expression::Member { object, property } = callee.as_ref() else {
                    unreachable!("filtered above");
                };
                if let Some(object_binding) = self.resolve_object_binding_from_expression(object) {
                    let has_next_method = object_binding_lookup_value(
                        &object_binding,
                        &Expression::String("next".to_string()),
                    )
                    .and_then(|value| self.resolve_function_binding_from_expression(value))
                    .is_some();
                    if self
                        .resolve_member_function_binding(object, property)
                        .is_some()
                        || self
                            .resolve_member_getter_binding(object, property)
                            .is_some()
                        || self.resolve_iterator_source_kind(object).is_some()
                        || has_next_method
                    {
                        return Some(object_binding);
                    }
                }
                if self.resolve_iterator_source_kind(object).is_some() {
                    Some(empty_object_value_binding())
                } else {
                    None
                }
            }
            Expression::Call { callee, arguments }
                if arguments.is_empty()
                    && matches!(
                        callee.as_ref(),
                        Expression::Member { object, property }
                            if matches!(object.as_ref(), Expression::Identifier(name) if name == "$262")
                                && matches!(property.as_ref(), Expression::String(name) if name == "createRealm")
                    ) =>
            {
                Some(empty_object_value_binding())
            }
            Expression::New { callee, arguments } => {
                if matches!(
                    callee.as_ref(),
                    Expression::Identifier(name) if native_error_runtime_value(name).is_some()
                ) {
                    let mut object_binding = empty_object_value_binding();
                    if let Expression::Identifier(name) = callee.as_ref() {
                        object_binding_set_property(
                            &mut object_binding,
                            Expression::String("name".to_string()),
                            Expression::String(name.clone()),
                        );
                    }
                    if let Some(
                        CallArgument::Expression(message_expression)
                        | CallArgument::Spread(message_expression),
                    ) = arguments.get(1)
                    {
                        let materialized_message =
                            self.materialize_static_expression(message_expression);
                        if !matches!(materialized_message, Expression::Undefined)
                            && !matches!(&materialized_message, Expression::Identifier(name)
                                if name == "undefined" && self.is_unshadowed_builtin_identifier(name))
                        {
                            object_binding_set_property(
                                &mut object_binding,
                                Expression::String("message".to_string()),
                                materialized_message,
                            );
                        }
                    }
                    Some(object_binding)
                } else {
                    self.resolve_user_constructor_object_binding_from_new(callee, arguments)
                        .or_else(|| {
                            self.last_bound_user_function_call
                                .as_ref()
                                .filter(|snapshot| {
                                    snapshot
                                        .source_expression
                                        .as_ref()
                                        .is_some_and(|source| {
                                            static_expression_matches(source, expression)
                                        })
                                })
                                .and_then(|snapshot| snapshot.result_expression.as_ref())
                                .and_then(|result| {
                                    self.resolve_object_binding_from_expression(result)
                                })
                        })
                        .or_else(|| {
                            (arguments.is_empty()
                                && matches!(callee.as_ref(), Expression::Identifier(name) if name == "Object"))
                            .then(empty_object_value_binding)
                        })
                        .or_else(|| {
                            matches!(callee.as_ref(), Expression::Identifier(name) if name == "WeakRef")
                                .then(empty_object_value_binding)
                        })
                }
            }
            Expression::Call { callee, arguments }
                if matches!(
                    callee.as_ref(),
                    Expression::Identifier(name) if native_error_runtime_value(name).is_some()
                ) =>
            {
                let mut object_binding = empty_object_value_binding();
                if let Expression::Identifier(name) = callee.as_ref() {
                    object_binding_set_property(
                        &mut object_binding,
                        Expression::String("name".to_string()),
                        Expression::String(name.clone()),
                    );
                }
                if let Some(
                    CallArgument::Expression(message_expression)
                    | CallArgument::Spread(message_expression),
                ) = arguments.get(1)
                {
                    let materialized_message =
                        self.materialize_static_expression(message_expression);
                    if !matches!(materialized_message, Expression::Undefined)
                        && !matches!(&materialized_message, Expression::Identifier(name)
                            if name == "undefined" && self.is_unshadowed_builtin_identifier(name))
                    {
                        object_binding_set_property(
                            &mut object_binding,
                            Expression::String("message".to_string()),
                            materialized_message,
                        );
                    }
                }
                Some(object_binding)
            }
            Expression::Call { callee, arguments } => self
                .resolve_static_call_result_expression_with_context(
                    callee,
                    arguments,
                    self.current_user_function_name.as_deref(),
                )
                .filter(|_| matches!(callee.as_ref(), Expression::Identifier(_)))
                .and_then(|(result_expression, _)| {
                    self.resolve_object_binding_from_expression(&result_expression)
                })
                .or_else(|| self.resolve_returned_object_binding_from_call(callee, arguments))
                .or_else(|| {
                    if !arguments.is_empty() {
                        return None;
                    }
                    let LocalFunctionBinding::User(function_name) = self
                        .resolve_function_binding_from_expression_with_context(
                            callee,
                            self.current_user_function_name.as_deref(),
                        )?
                    else {
                        return None;
                    };
                    let (result_expression, _) = self
                        .execute_simple_static_user_function_with_bindings(
                            &function_name,
                            &HashMap::new(),
                        )?;
                    self.resolve_object_binding_from_expression(&result_expression)
                }),
            Expression::Object(entries) => {
                let mut value_bindings = self.module.global_value_bindings.clone();
                value_bindings.extend(self.local_value_bindings.clone());
                let mut object_bindings = self.module.global_object_bindings.clone();
                object_bindings.extend(self.local_object_bindings.clone());
                self.resolve_object_binding_entries_with_state(
                    entries,
                    &mut HashMap::new(),
                    &mut value_bindings,
                    &mut object_bindings,
                )
            }
            _ => self
                .resolve_bound_alias_expression(expression)
                .filter(|resolved| resolved != expression)
                .and_then(|resolved| self.resolve_object_binding_from_expression(&resolved)),
        };
        if binding.is_some() {
            return binding;
        }

        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.resolve_object_binding_from_expression(&materialized);
        }
        None
    }
}
