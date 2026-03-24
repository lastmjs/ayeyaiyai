use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn expression_is_builtin_array_constructor(
        &self,
        expression: &Expression,
    ) -> bool {
        matches!(
            self.materialize_static_expression(expression),
            Expression::Identifier(name) if name == "Array"
        )
    }

    pub(in crate::backend::direct_wasm) fn expression_is_known_array_value(
        &self,
        expression: &Expression,
    ) -> bool {
        if self
            .resolve_array_binding_from_expression(expression)
            .is_some()
        {
            return true;
        }

        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression)
            && self
                .resolve_array_binding_from_expression(&materialized)
                .is_some()
        {
            return true;
        }

        self.resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
            .is_some_and(|resolved| self.expression_is_known_array_value(&resolved))
    }

    pub(in crate::backend::direct_wasm) fn emit_static_weakref_deref_call(
        &mut self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let target = match callee {
            Expression::Member { object, property }
                if matches!(property.as_ref(), Expression::String(name) if name == "deref")
                    && arguments.is_empty() =>
            {
                self.emit_numeric_expression(object)?;
                self.instructions.push(0x1a);
                self.resolve_static_weakref_target_expression(object)
            }
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "call") =>
            {
                let Expression::Member {
                    object: deref_target,
                    property: deref_property,
                } = object.as_ref()
                else {
                    return Ok(false);
                };
                if !matches!(deref_property.as_ref(), Expression::String(name) if name == "deref") {
                    return Ok(false);
                }
                self.emit_numeric_expression(deref_target)?;
                self.instructions.push(0x1a);
                let target = match arguments.first() {
                    Some(CallArgument::Expression(this_expression))
                    | Some(CallArgument::Spread(this_expression)) => {
                        self.resolve_static_weakref_target_expression(this_expression)
                    }
                    None => None,
                };
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.emit_numeric_expression(expression)?;
                            self.instructions.push(0x1a);
                        }
                    }
                }
                target
            }
            _ => return Ok(false),
        };
        let Some(target) = target else {
            return Ok(false);
        };
        self.emit_numeric_expression(&target)?;
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_instanceof_truthy_from_local(
        &mut self,
        value_local: u32,
    ) -> DirectResult<()> {
        self.push_local_get(value_local);
        self.push_i32_const(0);
        self.push_binary_op(BinaryOp::NotEqual)?;

        self.push_local_get(value_local);
        self.push_i32_const(JS_NULL_TAG);
        self.push_binary_op(BinaryOp::NotEqual)?;
        self.instructions.push(0x71);

        self.push_local_get(value_local);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.push_binary_op(BinaryOp::NotEqual)?;
        self.instructions.push(0x71);

        self.push_local_get(value_local);
        self.push_i32_const(JS_NAN_TAG);
        self.push_binary_op(BinaryOp::NotEqual)?;
        self.instructions.push(0x71);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn expression_is_known_non_object_value_for_instanceof(
        &self,
        expression: &Expression,
    ) -> bool {
        if self.expression_is_known_array_value(expression)
            || self.expression_is_known_function_value_for_instanceof(expression)
            || self.expression_is_known_promise_instance_for_instanceof(expression)
            || self.expression_is_known_constructor_instance_for_instanceof(expression, "WeakRef")
            || self.expression_is_known_native_error_instance_for_instanceof(expression, "Error")
        {
            return false;
        }
        if let Some(resolved) = self
            .resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
        {
            return self.expression_is_known_non_object_value_for_instanceof(&resolved);
        }
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.expression_is_known_non_object_value_for_instanceof(&materialized);
        }
        matches!(
            self.infer_value_kind(&materialized),
            Some(
                StaticValueKind::Number
                    | StaticValueKind::Bool
                    | StaticValueKind::String
                    | StaticValueKind::BigInt
                    | StaticValueKind::Symbol
                    | StaticValueKind::Null
                    | StaticValueKind::Undefined
            )
        )
    }

    pub(in crate::backend::direct_wasm) fn expression_is_known_function_value_for_instanceof(
        &self,
        expression: &Expression,
    ) -> bool {
        if self
            .resolve_function_binding_from_expression(expression)
            .is_some()
        {
            return true;
        }
        if let Some(resolved) = self
            .resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
        {
            return self.expression_is_known_function_value_for_instanceof(&resolved);
        }
        if matches!(
            expression,
            Expression::Call { callee, .. }
                if matches!(callee.as_ref(), Expression::Identifier(name)
                    if is_function_constructor_builtin(name))
        ) {
            return true;
        }
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.expression_is_known_function_value_for_instanceof(&materialized);
        }
        matches!(
            self.infer_value_kind(&materialized),
            Some(StaticValueKind::Function)
        ) || matches!(
            materialized,
            Expression::Call { ref callee, .. }
                if matches!(callee.as_ref(), Expression::Identifier(name)
                    if is_function_constructor_builtin(name))
        )
    }

    pub(in crate::backend::direct_wasm) fn expression_is_known_promise_instance_for_instanceof(
        &self,
        expression: &Expression,
    ) -> bool {
        if let Some(resolved) = self
            .resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
        {
            return self.expression_is_known_promise_instance_for_instanceof(&resolved);
        }
        match expression {
            Expression::New { callee, .. } => {
                return matches!(callee.as_ref(), Expression::Identifier(name) if name == "Promise");
            }
            Expression::Call { callee, .. } => {
                if matches!(callee.as_ref(), Expression::Identifier(name) if name == "Promise") {
                    return true;
                }
                if matches!(
                    callee.as_ref(),
                    Expression::Member { object, property }
                        if matches!(object.as_ref(), Expression::Identifier(name) if name == "Promise")
                            && matches!(property.as_ref(), Expression::String(name) if name == "resolve")
                ) {
                    return true;
                }
                if self
                    .resolve_user_function_from_expression(callee.as_ref())
                    .is_some_and(|user_function| user_function.is_async())
                {
                    return true;
                }
            }
            _ => {}
        }
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.expression_is_known_promise_instance_for_instanceof(&materialized);
        }
        match materialized {
            Expression::New { callee, .. } => {
                matches!(callee.as_ref(), Expression::Identifier(name) if name == "Promise")
            }
            Expression::Call { callee, .. } => {
                if matches!(callee.as_ref(), Expression::Identifier(name) if name == "Promise") {
                    return true;
                }
                if matches!(
                    callee.as_ref(),
                    Expression::Member { object, property }
                        if matches!(object.as_ref(), Expression::Identifier(name) if name == "Promise")
                            && matches!(property.as_ref(), Expression::String(name) if name == "resolve")
                ) {
                    return true;
                }
                self.resolve_user_function_from_expression(callee.as_ref())
                    .is_some_and(|user_function| user_function.is_async())
            }
            _ => false,
        }
    }

    pub(in crate::backend::direct_wasm) fn expression_is_known_constructor_instance_for_instanceof(
        &self,
        expression: &Expression,
        constructor_name: &str,
    ) -> bool {
        if let Some(resolved) = self
            .resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
        {
            return self.expression_is_known_constructor_instance_for_instanceof(
                &resolved,
                constructor_name,
            );
        }
        match expression {
            Expression::New { callee, .. } => {
                return matches!(callee.as_ref(), Expression::Identifier(name) if name == constructor_name);
            }
            Expression::Call { callee, .. } => {
                return matches!(callee.as_ref(), Expression::Identifier(name) if name == constructor_name)
                    && (constructor_name == "AggregateError"
                        || native_error_runtime_value(constructor_name).is_some());
            }
            _ => {}
        }
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.expression_is_known_constructor_instance_for_instanceof(
                &materialized,
                constructor_name,
            );
        }
        match materialized {
            Expression::New { callee, .. } => {
                matches!(callee.as_ref(), Expression::Identifier(name) if name == constructor_name)
            }
            Expression::Call { callee, .. } => {
                matches!(callee.as_ref(), Expression::Identifier(name) if name == constructor_name)
                    && (constructor_name == "AggregateError"
                        || native_error_runtime_value(constructor_name).is_some())
            }
            _ => false,
        }
    }

    pub(in crate::backend::direct_wasm) fn expression_is_known_native_error_instance_for_instanceof(
        &self,
        expression: &Expression,
        constructor_name: &str,
    ) -> bool {
        if constructor_name == "Error" {
            return NATIVE_ERROR_NAMES.iter().any(|candidate| {
                self.expression_is_known_constructor_instance_for_instanceof(expression, candidate)
            });
        }
        self.expression_is_known_constructor_instance_for_instanceof(expression, constructor_name)
    }

    pub(in crate::backend::direct_wasm) fn expression_is_known_object_like_value_for_instanceof(
        &self,
        expression: &Expression,
    ) -> bool {
        if self.expression_is_known_array_value(expression)
            || self.expression_is_known_function_value_for_instanceof(expression)
            || self.expression_is_known_promise_instance_for_instanceof(expression)
            || self.expression_is_known_constructor_instance_for_instanceof(expression, "WeakRef")
            || self.expression_is_known_native_error_instance_for_instanceof(expression, "Error")
        {
            return true;
        }
        if let Some(resolved) = self
            .resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
        {
            return self.expression_is_known_object_like_value_for_instanceof(&resolved);
        }
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.expression_is_known_object_like_value_for_instanceof(&materialized);
        }
        matches!(
            self.infer_value_kind(&materialized),
            Some(StaticValueKind::Object)
        )
    }

    pub(in crate::backend::direct_wasm) fn expression_inherits_from_prototype_for_instanceof(
        &self,
        left: &Expression,
        prototype: &Expression,
    ) -> bool {
        if let Some(resolved) = self
            .resolve_bound_alias_expression(prototype)
            .filter(|resolved| !static_expression_matches(resolved, prototype))
        {
            return self.expression_inherits_from_prototype_for_instanceof(left, &resolved);
        }
        let materialized_prototype = self.materialize_static_expression(prototype);
        if !static_expression_matches(&materialized_prototype, prototype) {
            return self
                .expression_inherits_from_prototype_for_instanceof(left, &materialized_prototype);
        }
        let Expression::Member { object, property } = &materialized_prototype else {
            return false;
        };
        if !matches!(property.as_ref(), Expression::String(name) if name == "prototype") {
            return false;
        }
        let Expression::Identifier(constructor_name) = object.as_ref() else {
            return false;
        };
        match constructor_name.as_str() {
            "Array" => self.expression_is_known_array_value(left),
            "Function" | "AsyncFunction" | "GeneratorFunction" | "AsyncGeneratorFunction" => {
                self.expression_is_known_function_value_for_instanceof(left)
            }
            "Object" => self.expression_is_known_object_like_value_for_instanceof(left),
            "Promise" => self.expression_is_known_promise_instance_for_instanceof(left),
            "WeakRef" => {
                self.expression_is_known_constructor_instance_for_instanceof(left, "WeakRef")
            }
            "Error" => self.expression_is_known_native_error_instance_for_instanceof(left, "Error"),
            name if native_error_runtime_value(name).is_some() => {
                self.expression_is_known_native_error_instance_for_instanceof(left, name)
            }
            name => self.expression_is_known_constructor_instance_for_instanceof(left, name),
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_instanceof_prototype_expression(
        &self,
        right: &Expression,
    ) -> Option<Expression> {
        let prototype_property = Expression::String("prototype".to_string());
        if let Some(binding) = self.resolve_member_getter_binding(right, &prototype_property) {
            return self.resolve_function_binding_static_return_expression_with_call_frame(
                &binding,
                &[],
                right,
            );
        }
        if let Some(object_binding) = self.resolve_object_binding_from_expression(right)
            && let Some(value) =
                object_binding_lookup_value(&object_binding, &prototype_property).cloned()
        {
            return Some(value);
        }
        let materialized_right = self.materialize_static_expression(right);
        if !static_expression_matches(&materialized_right, right) {
            return self.resolve_instanceof_prototype_expression(&materialized_right);
        }
        if matches!(
            self.resolve_function_binding_from_expression(&materialized_right),
            Some(_)
        ) || matches!(
            &materialized_right,
            Expression::Identifier(name) if infer_call_result_kind(name).is_some()
        ) {
            return Some(Expression::Member {
                object: Box::new(materialized_right),
                property: Box::new(prototype_property),
            });
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn emit_instanceof_expression(
        &mut self,
        left: &Expression,
        right: &Expression,
    ) -> DirectResult<()> {
        let has_instance_property = Expression::Member {
            object: Box::new(Expression::Identifier("Symbol".to_string())),
            property: Box::new(Expression::String("hasInstance".to_string())),
        };
        if let Some(function_binding) =
            self.resolve_member_function_binding(right, &has_instance_property)
        {
            let left_local = self.allocate_temp_local();
            self.emit_numeric_expression(left)?;
            self.push_local_set(left_local);
            let result_local = self.allocate_temp_local();
            match function_binding {
                LocalFunctionBinding::User(function_name) => {
                    let Some(user_function) =
                        self.module.user_function_map.get(&function_name).cloned()
                    else {
                        self.push_i32_const(0);
                        return Ok(());
                    };
                    let argument_locals = [left_local];
                    if let Some(capture_slots) =
                        self.resolve_member_function_capture_slots(right, &has_instance_property)
                    {
                        self.emit_user_function_call_with_new_target_and_this_expression_and_bound_captures_from_argument_locals(
                            &user_function,
                            &argument_locals,
                            1,
                            JS_UNDEFINED_TAG,
                            right,
                            &capture_slots,
                        )?;
                    } else {
                        self.emit_user_function_call_with_new_target_and_this_expression_from_argument_locals(
                            &user_function,
                            &argument_locals,
                            1,
                            JS_UNDEFINED_TAG,
                            right,
                        )?;
                    }
                    self.push_local_set(result_local);
                    self.emit_instanceof_truthy_from_local(result_local)?;
                    return Ok(());
                }
                LocalFunctionBinding::Builtin(_) => {
                    self.emit_numeric_expression(right)?;
                    self.instructions.push(0x1a);
                    self.push_i32_const(0);
                    return Ok(());
                }
            }
        }

        let materialized_right = self.materialize_static_expression(right);
        if self.expression_is_builtin_array_constructor(&materialized_right) {
            self.emit_numeric_expression(left)?;
            self.instructions.push(0x1a);
            self.emit_numeric_expression(right)?;
            self.instructions.push(0x1a);
            self.push_i32_const(if self.expression_is_known_array_value(left) {
                1
            } else {
                0
            });
            return Ok(());
        }

        if let Expression::Identifier(name) = &materialized_right {
            if let Some(expected_values) = native_error_instanceof_values(name) {
                let left_local = self.allocate_temp_local();
                self.emit_numeric_expression(left)?;
                self.push_local_set(left_local);
                self.emit_numeric_expression(right)?;
                self.instructions.push(0x1a);
                if let [expected_value] = expected_values.as_slice() {
                    self.push_local_get(left_local);
                    self.push_i32_const(*expected_value);
                    self.push_binary_op(BinaryOp::Equal)?;
                    return Ok(());
                }

                let matched_local = self.allocate_temp_local();
                self.push_i32_const(0);
                self.push_local_set(matched_local);
                for expected_value in expected_values {
                    self.push_local_get(left_local);
                    self.push_i32_const(expected_value);
                    self.push_binary_op(BinaryOp::Equal)?;
                    self.instructions.push(0x04);
                    self.instructions.push(EMPTY_BLOCK_TYPE);
                    self.push_control_frame();
                    self.push_i32_const(1);
                    self.push_local_set(matched_local);
                    self.instructions.push(0x0b);
                    self.pop_control_frame();
                }
                self.push_local_get(matched_local);
                return Ok(());
            }
        }

        if let Some(prototype_expression) =
            self.resolve_instanceof_prototype_expression(&materialized_right)
        {
            let left_local = self.allocate_temp_local();
            self.emit_numeric_expression(left)?;
            self.push_local_set(left_local);
            let static_result = if self.expression_is_known_non_object_value_for_instanceof(left) {
                false
            } else {
                self.expression_inherits_from_prototype_for_instanceof(left, &prototype_expression)
            };
            if let Some(getter_binding) = self.resolve_member_getter_binding(
                &materialized_right,
                &Expression::String("prototype".to_string()),
            ) {
                match getter_binding {
                    LocalFunctionBinding::User(function_name) => {
                        if let Some(user_function) =
                            self.module.user_function_map.get(&function_name).cloned()
                        {
                            self.emit_user_function_call_with_new_target_and_this_expression_from_argument_locals(
                                &user_function,
                                &[],
                                0,
                                JS_UNDEFINED_TAG,
                                &materialized_right,
                            )?;
                            self.instructions.push(0x1a);
                        } else {
                            self.emit_numeric_expression(&materialized_right)?;
                            self.instructions.push(0x1a);
                        }
                    }
                    LocalFunctionBinding::Builtin(function_name) => {
                        let getter_callee = Expression::Identifier(function_name);
                        if !self.emit_arguments_slot_accessor_call(
                            &getter_callee,
                            &[],
                            0,
                            Some(&[]),
                        )? {
                            self.push_i32_const(JS_UNDEFINED_TAG);
                        }
                        self.instructions.push(0x1a);
                    }
                }
            } else {
                self.emit_numeric_expression(right)?;
                self.instructions.push(0x1a);
            }
            self.push_i32_const(if static_result { 1 } else { 0 });
            return Ok(());
        }

        self.emit_numeric_expression(left)?;
        self.instructions.push(0x1a);
        self.emit_numeric_expression(right)?;
        self.instructions.push(0x1a);
        self.push_i32_const(0);
        Ok(())
    }
}
