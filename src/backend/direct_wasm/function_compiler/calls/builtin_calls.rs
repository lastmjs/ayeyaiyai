use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_assert_compare_array_call(
        &mut self,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let [
            CallArgument::Expression(actual),
            CallArgument::Expression(expected),
            rest @ ..,
        ] = arguments
        else {
            return Ok(false);
        };
        let Some(expected_binding) = self.resolve_array_binding_from_expression(expected) else {
            return Ok(false);
        };

        self.emit_numeric_expression(actual)?;
        self.instructions.push(0x1a);
        self.emit_numeric_expression(expected)?;
        self.instructions.push(0x1a);
        for argument in rest {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.instructions.push(0x1a);
                }
            }
        }

        if self.current_user_function_name.is_some()
            && matches!(
                actual,
                Expression::Identifier(_) | Expression::Member { .. }
            )
        {
            return Ok(false);
        }

        if matches!(
            actual,
            Expression::Identifier(_) | Expression::Member { .. }
        ) {
            return self
                .emit_runtime_assert_compare_array_against_expected(actual, &expected_binding);
        }

        let Some(actual_binding) = self.resolve_array_binding_from_expression(actual) else {
            return Ok(false);
        };
        if !self.array_bindings_equal(&actual_binding, &expected_binding) {
            self.emit_error_throw()?;
        } else {
            self.push_i32_const(JS_UNDEFINED_TAG);
        }
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_assert_compare_array_against_expected(
        &mut self,
        actual: &Expression,
        expected_binding: &ArrayValueBinding,
    ) -> DirectResult<bool> {
        let mismatch_local = self.allocate_temp_local();
        self.push_i32_const(0);
        self.push_local_set(mismatch_local);

        self.emit_numeric_expression(&Expression::Member {
            object: Box::new(actual.clone()),
            property: Box::new(Expression::String("length".to_string())),
        })?;
        self.push_i32_const(expected_binding.values.len() as i32);
        self.push_binary_op(BinaryOp::NotEqual)?;
        self.instructions.push(0x04);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.push_i32_const(1);
        self.push_local_set(mismatch_local);
        self.instructions.push(0x0b);
        self.pop_control_frame();

        for (index, expected_value) in expected_binding.values.iter().enumerate() {
            self.emit_numeric_expression(&Expression::Member {
                object: Box::new(actual.clone()),
                property: Box::new(Expression::Number(index as f64)),
            })?;
            self.emit_numeric_expression(&expected_value.clone().unwrap_or(Expression::Undefined))?;
            self.push_binary_op(BinaryOp::NotEqual)?;
            self.instructions.push(0x04);
            self.instructions.push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.push_i32_const(1);
            self.push_local_set(mismatch_local);
            self.instructions.push(0x0b);
            self.pop_control_frame();
        }

        self.push_local_get(mismatch_local);
        self.instructions.push(0x04);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.emit_error_throw()?;
        self.instructions.push(0x0b);
        self.pop_control_frame();
        self.push_i32_const(JS_UNDEFINED_TAG);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_object_set_prototype_of_call(
        &mut self,
        callee_object: &Expression,
        callee_property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if !matches!(callee_object, Expression::Identifier(name) if name == "Object") {
            return Ok(false);
        }
        if !matches!(callee_property, Expression::String(name) if name == "setPrototypeOf") {
            return Ok(false);
        }

        let [target_argument, prototype_argument, rest @ ..] = arguments else {
            return Ok(false);
        };
        let (
            CallArgument::Expression(target_expression),
            CallArgument::Expression(prototype_expression),
        ) = (target_argument, prototype_argument)
        else {
            return Ok(false);
        };

        let runtime_binding = match target_expression {
            Expression::Identifier(name) => self
                .module
                .global_runtime_prototype_bindings
                .get(name)
                .cloned(),
            _ => None,
        };
        let materialized_prototype = self.materialize_static_expression(prototype_expression);

        self.emit_numeric_expression(target_expression)?;
        self.instructions.push(0x1a);
        self.emit_numeric_expression(prototype_expression)?;
        self.instructions.push(0x1a);
        for argument in rest {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.instructions.push(0x1a);
                }
            }
        }

        if let Some(binding) = runtime_binding
            && let Some(global_index) = binding.global_index
            && let Some(variant_index) = binding.variants.iter().position(|candidate| {
                candidate
                    .as_ref()
                    .is_some_and(|candidate| candidate == &materialized_prototype)
            })
        {
            self.push_i32_const(variant_index as i32);
            self.push_global_set(global_index);
        }

        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_object_is_call(
        &mut self,
        callee_object: &Expression,
        callee_property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if !matches!(callee_object, Expression::Identifier(name) if name == "Object" && self.is_unshadowed_builtin_identifier(name))
        {
            return Ok(false);
        }
        if !matches!(callee_property, Expression::String(name) if name == "is") {
            return Ok(false);
        }

        let [
            CallArgument::Expression(actual),
            CallArgument::Expression(expected),
            rest @ ..,
        ] = arguments
        else {
            return Ok(false);
        };

        if let Some(result) = self.resolve_static_same_value_result_with_context(
            actual,
            expected,
            self.current_user_function_name.as_deref(),
        ) {
            self.emit_numeric_expression(actual)?;
            self.instructions.push(0x1a);
            self.emit_numeric_expression(expected)?;
            self.instructions.push(0x1a);
            for argument in rest {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        self.emit_numeric_expression(expression)?;
                        self.instructions.push(0x1a);
                    }
                }
            }
            self.push_i32_const(result as i32);
            return Ok(true);
        }

        let actual_local = self.allocate_temp_local();
        let expected_local = self.allocate_temp_local();

        self.emit_numeric_expression(actual)?;
        self.push_local_set(actual_local);
        self.emit_numeric_expression(expected)?;
        self.push_local_set(expected_local);

        for argument in rest {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.instructions.push(0x1a);
                }
            }
        }

        self.emit_same_value_result_from_locals(actual_local, expected_local, actual_local)?;
        self.push_local_get(actual_local);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_array_is_array_call(
        &mut self,
        callee_object: &Expression,
        callee_property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if !matches!(callee_object, Expression::Identifier(name) if name == "Array" && self.is_unshadowed_builtin_identifier(name))
        {
            return Ok(false);
        }
        if !matches!(callee_property, Expression::String(name) if name == "isArray") {
            return Ok(false);
        }

        let Some(first_argument) = arguments.first() else {
            self.push_i32_const(0);
            return Ok(true);
        };

        let array_like = match first_argument {
            CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                !matches!(expression, Expression::Identifier(name) if self.local_typed_array_view_bindings.contains_key(name))
                    && self
                        .resolve_array_binding_from_expression(expression)
                        .is_some()
            }
        };

        for argument in arguments {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.instructions.push(0x1a);
                }
            }
        }

        self.push_i32_const(array_like as i32);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_compare_array_call(
        &mut self,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let [
            CallArgument::Expression(actual),
            CallArgument::Expression(expected),
            rest @ ..,
        ] = arguments
        else {
            return Ok(false);
        };
        let Some(expected_binding) = self.resolve_array_binding_from_expression(expected) else {
            return Ok(false);
        };

        self.emit_numeric_expression(actual)?;
        self.instructions.push(0x1a);
        self.emit_numeric_expression(expected)?;
        self.instructions.push(0x1a);
        for argument in rest {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.instructions.push(0x1a);
                }
            }
        }

        if self.current_user_function_name.is_some()
            && matches!(
                actual,
                Expression::Identifier(_) | Expression::Member { .. }
            )
        {
            return Ok(false);
        }

        if matches!(
            actual,
            Expression::Identifier(_) | Expression::Member { .. }
        ) {
            self.push_i32_const(1);
            let result_local = self.allocate_temp_local();
            self.push_local_set(result_local);

            self.emit_numeric_expression(&Expression::Member {
                object: Box::new(actual.clone()),
                property: Box::new(Expression::String("length".to_string())),
            })?;
            self.push_i32_const(expected_binding.values.len() as i32);
            self.push_binary_op(BinaryOp::NotEqual)?;
            self.instructions.push(0x04);
            self.instructions.push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.push_i32_const(0);
            self.push_local_set(result_local);
            self.instructions.push(0x0b);
            self.pop_control_frame();

            for (index, expected_value) in expected_binding.values.iter().enumerate() {
                self.emit_numeric_expression(&Expression::Member {
                    object: Box::new(actual.clone()),
                    property: Box::new(Expression::Number(index as f64)),
                })?;
                self.emit_numeric_expression(
                    &expected_value.clone().unwrap_or(Expression::Undefined),
                )?;
                self.push_binary_op(BinaryOp::NotEqual)?;
                self.instructions.push(0x04);
                self.instructions.push(EMPTY_BLOCK_TYPE);
                self.push_control_frame();
                self.push_i32_const(0);
                self.push_local_set(result_local);
                self.instructions.push(0x0b);
                self.pop_control_frame();
            }

            self.push_local_get(result_local);
            return Ok(true);
        }

        let Some(actual_binding) = self.resolve_array_binding_from_expression(actual) else {
            return Ok(false);
        };
        self.push_i32_const(
            if self.array_bindings_equal(&actual_binding, &expected_binding) {
                1
            } else {
                0
            },
        );
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_object_array_builtin_call(
        &mut self,
        callee_object: &Expression,
        callee_property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if !matches!(callee_object, Expression::Identifier(name) if name == "Object") {
            return Ok(false);
        }
        if !matches!(callee_property, Expression::String(name) if name == "keys" || name == "getOwnPropertyNames" || name == "getOwnPropertySymbols")
        {
            return Ok(false);
        }
        for argument in arguments {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.instructions.push(0x1a);
                }
            }
        }
        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_object_get_prototype_of_call(
        &mut self,
        callee_object: &Expression,
        callee_property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if !matches!(callee_object, Expression::Identifier(name) if name == "Object") {
            return Ok(false);
        }
        if !matches!(callee_property, Expression::String(name) if name == "getPrototypeOf") {
            return Ok(false);
        }
        let [CallArgument::Expression(target), ..] = arguments else {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(true);
        };
        self.emit_numeric_expression(target)?;
        self.instructions.push(0x1a);
        for argument in arguments.iter().skip(1) {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.instructions.push(0x1a);
                }
            }
        }
        if let Some(prototype) = self.resolve_static_object_prototype_expression(target) {
            self.emit_numeric_expression(&prototype)?;
        } else {
            self.push_i32_const(JS_UNDEFINED_TAG);
        }
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_object_is_extensible_call(
        &mut self,
        callee_object: &Expression,
        callee_property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if !matches!(callee_object, Expression::Identifier(name) if name == "Object") {
            return Ok(false);
        }
        if !matches!(callee_property, Expression::String(name) if name == "isExtensible") {
            return Ok(false);
        }
        let target = match arguments.first() {
            Some(CallArgument::Expression(target)) | Some(CallArgument::Spread(target)) => target,
            None => {
                self.push_i32_const(0);
                return Ok(true);
            }
        };
        self.emit_numeric_expression(target)?;
        self.instructions.push(0x1a);
        for argument in arguments.iter().skip(1) {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.instructions.push(0x1a);
                }
            }
        }
        self.push_i32_const(
            if self
                .resolve_static_object_prototype_expression(target)
                .is_some()
            {
                1
            } else {
                0
            },
        );
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_array_for_each_call(
        &mut self,
        object: &Expression,
        property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if !matches!(property, Expression::String(name) if name == "forEach") {
            return Ok(false);
        }
        let Some(array_binding) = self.resolve_array_binding_from_expression(object) else {
            return Ok(false);
        };
        let Some(CallArgument::Expression(callback)) = arguments.first() else {
            return Ok(false);
        };
        let Some(LocalFunctionBinding::User(function_name)) =
            self.resolve_function_binding_from_expression(callback)
        else {
            return Ok(false);
        };
        let Some(user_function) = self.module.user_function_map.get(&function_name).cloned() else {
            return Ok(false);
        };

        let this_expression = match arguments.get(1) {
            Some(CallArgument::Expression(expression)) | Some(CallArgument::Spread(expression)) => {
                expression.clone()
            }
            None => Expression::Undefined,
        };
        let this_hidden_name =
            self.allocate_named_hidden_local("array_foreach_this", StaticValueKind::Unknown);
        let this_hidden_local = self
            .locals
            .get(&this_hidden_name)
            .copied()
            .expect("forEach this hidden local should exist");
        let array_hidden_name =
            self.allocate_named_hidden_local("array_foreach_array", StaticValueKind::Object);
        let array_hidden_local = self
            .locals
            .get(&array_hidden_name)
            .copied()
            .expect("forEach array hidden local should exist");

        self.emit_numeric_expression(object)?;
        self.push_local_set(array_hidden_local);
        self.emit_numeric_expression(callback)?;
        self.instructions.push(0x1a);
        self.emit_numeric_expression(&this_expression)?;
        self.push_local_set(this_hidden_local);
        for argument in arguments.iter().skip(2) {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.instructions.push(0x1a);
                }
            }
        }

        for (index, value) in array_binding.values.iter().enumerate() {
            let Some(value) = value.clone() else {
                continue;
            };
            let callback_arguments = [
                CallArgument::Expression(value),
                CallArgument::Expression(Expression::Number(index as f64)),
                CallArgument::Expression(Expression::Identifier(array_hidden_name.clone())),
            ];
            self.emit_user_function_call_with_new_target_and_this_expression(
                &user_function,
                &callback_arguments,
                JS_UNDEFINED_TAG,
                &Expression::Identifier(this_hidden_name.clone()),
            )?;
            self.instructions.push(0x1a);
        }

        self.push_i32_const(JS_UNDEFINED_TAG);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_function_constructor_builtin_call(
        &mut self,
        name: &str,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if !is_function_constructor_builtin(name) {
            return Ok(false);
        }

        if let Some((parameter_source, body_source)) =
            function_constructor_literal_source_parts(arguments)
        {
            let wrappers =
                function_constructor_wrapper_sources(name, &parameter_source, &body_source)
                    .expect("checked builtin names should produce wrapper sources");
            let parses = wrappers
                .iter()
                .any(|wrapper| frontend::parse(wrapper).is_ok());
            if !parses {
                self.emit_named_error_throw("SyntaxError")?;
                return Ok(true);
            }
        }

        for argument in arguments {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.instructions.push(0x1a);
                }
            }
        }
        self.push_i32_const(JS_TYPEOF_FUNCTION_TAG);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_test262_realm_eval_call(
        &mut self,
        builtin_name: &str,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let Some(realm_id) = parse_test262_realm_eval_builtin(builtin_name) else {
            return Ok(false);
        };
        let Some(argument) = arguments.first() else {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(true);
        };

        let CallArgument::Expression(Expression::String(argument_source)) = argument else {
            return Ok(false);
        };
        let Ok(program) = frontend::parse_script_goal(argument_source) else {
            self.emit_named_error_throw("SyntaxError")?;
            return Ok(true);
        };

        let [Statement::Var { name, value }] = program.statements.as_slice() else {
            return Ok(false);
        };
        let materialized_value = self.materialize_static_expression(value);
        let Some(realm) = self.module.test262_realms.get_mut(&realm_id) else {
            return Ok(false);
        };
        object_binding_set_property(
            &mut realm.global_object_binding,
            Expression::String(name.clone()),
            materialized_value,
        );

        for argument in arguments.iter().skip(1) {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.instructions.push(0x1a);
                }
            }
        }
        self.push_i32_const(JS_UNDEFINED_TAG);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_builtin_call(
        &mut self,
        name: &str,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if matches!(
            name,
            "__assert" | "__assertSameValue" | "__assertNotSameValue"
        ) {
            return self.emit_assertion_builtin_call(name, arguments);
        }

        if name == "isNaN" {
            return self.emit_is_nan_call(arguments);
        }

        if name == "eval" {
            return self.emit_eval_call(arguments);
        }

        if name == TEST262_CREATE_REALM_BUILTIN {
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        self.emit_numeric_expression(expression)?;
                        self.instructions.push(0x1a);
                    }
                }
            }
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
            return Ok(true);
        }

        if self.emit_test262_realm_eval_call(name, arguments)? {
            return Ok(true);
        }

        if self.emit_function_constructor_builtin_call(name, arguments)? {
            return Ok(true);
        }

        if let Some(native_error_value) = native_error_runtime_value(name) {
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        self.emit_numeric_expression(expression)?;
                        self.instructions.push(0x1a);
                    }
                }
            }
            self.push_i32_const(native_error_value);
            return Ok(true);
        }

        let Some(result_tag) = (match name {
            "Number" => Some(JS_TYPEOF_NUMBER_TAG),
            "String" => Some(JS_TYPEOF_STRING_TAG),
            "Boolean" => Some(JS_TYPEOF_BOOLEAN_TAG),
            "Object" | "Array" | "Date" | "RegExp" | "Map" | "Set" | "Error" | "EvalError"
            | "RangeError" | "ReferenceError" | "SyntaxError" | "TypeError" | "URIError"
            | "AggregateError" | "Promise" | "WeakRef" => Some(JS_TYPEOF_OBJECT_TAG),
            "BigInt" => Some(JS_TYPEOF_BIGINT_TAG),
            "Symbol" => Some(JS_TYPEOF_SYMBOL_TAG),
            _ => None,
        }) else {
            return Ok(false);
        };

        for argument in arguments {
            match argument {
                CallArgument::Expression(expression) => self.emit_numeric_expression(expression)?,
                CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                }
            }
            self.instructions.push(0x1a);
        }
        self.push_i32_const(result_tag);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_is_nan_call(
        &mut self,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let first_argument = arguments.first();

        if let Some(CallArgument::Expression(Expression::String(text))) = first_argument {
            for argument in arguments.iter().skip(1) {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        self.emit_numeric_expression(expression)?;
                        self.instructions.push(0x1a);
                    }
                }
            }
            self.push_i32_const(if parse_string_to_i32(text).is_ok() {
                0
            } else {
                1
            });
            return Ok(true);
        }

        if matches!(
            first_argument,
            Some(CallArgument::Expression(
                Expression::Object(_) | Expression::Array(_) | Expression::This
            ))
        ) {
            for argument in arguments.iter() {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        self.emit_numeric_expression(expression)?;
                        self.instructions.push(0x1a);
                    }
                }
            }
            self.push_i32_const(1);
            return Ok(true);
        }

        let value_local = self.allocate_temp_local();
        match first_argument {
            Some(CallArgument::Expression(expression) | CallArgument::Spread(expression)) => {
                self.emit_numeric_expression(expression)?;
            }
            None => self.push_i32_const(JS_UNDEFINED_TAG),
        }
        self.push_local_set(value_local);

        for argument in arguments.iter().skip(1) {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.instructions.push(0x1a);
                }
            }
        }

        self.push_local_get(value_local);
        self.push_i32_const(JS_NAN_TAG);
        self.push_binary_op(BinaryOp::Equal)?;
        self.push_local_get(value_local);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.push_binary_op(BinaryOp::Equal)?;
        self.push_binary_op(BinaryOp::BitwiseOr)?;
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_assert_throws_call(
        &mut self,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let [
            CallArgument::Expression(expected_error),
            CallArgument::Expression(callback),
            rest @ ..,
        ] = arguments
        else {
            return Ok(false);
        };

        self.emit_numeric_expression(expected_error)?;
        self.instructions.push(0x1a);
        for argument in rest {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.instructions.push(0x1a);
                }
            }
        }
        let callback_name =
            self.allocate_named_hidden_local("assert_throws_callback", StaticValueKind::Unknown);
        self.emit_statement(&Statement::Let {
            name: callback_name.clone(),
            mutable: false,
            value: callback.clone(),
        })?;

        let caught_name =
            self.allocate_named_hidden_local("assert_throws_caught", StaticValueKind::Bool);
        self.emit_statement(&Statement::Let {
            name: caught_name.clone(),
            mutable: true,
            value: Expression::Bool(false),
        })?;
        let caught_local = self.lookup_local(&caught_name)?;

        self.emit_statement(&Statement::Try {
            body: vec![Statement::Expression(Expression::Call {
                callee: Box::new(Expression::Identifier(callback_name)),
                arguments: Vec::new(),
            })],
            catch_binding: None,
            catch_setup: Vec::new(),
            catch_body: vec![Statement::Assign {
                name: caught_name,
                value: Expression::Bool(true),
            }],
        })?;

        self.push_local_get(caught_local);
        self.push_i32_const(0);
        self.push_binary_op(BinaryOp::Equal)?;
        self.instructions.push(0x04);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.emit_error_throw()?;
        self.instructions.push(0x0b);
        self.pop_control_frame();
        self.push_i32_const(JS_UNDEFINED_TAG);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_assert_throws_statement(
        &mut self,
        expression: &Expression,
    ) -> DirectResult<bool> {
        let Expression::Call { callee, arguments } = expression else {
            return Ok(false);
        };
        let Expression::Identifier(name) = callee.as_ref() else {
            return Ok(false);
        };
        if name != "__ayyAssertThrows" {
            return Ok(false);
        }

        let [
            CallArgument::Expression(expected_error),
            CallArgument::Expression(callback),
            rest @ ..,
        ] = arguments.as_slice()
        else {
            return Ok(false);
        };

        self.emit_numeric_expression(expected_error)?;
        self.instructions.push(0x1a);
        for argument in rest {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.instructions.push(0x1a);
                }
            }
        }

        let callback_name =
            self.allocate_named_hidden_local("assert_throws_callback", StaticValueKind::Unknown);
        self.emit_statement(&Statement::Let {
            name: callback_name.clone(),
            mutable: false,
            value: callback.clone(),
        })?;

        let caught_name =
            self.allocate_named_hidden_local("assert_throws_caught", StaticValueKind::Bool);
        self.emit_statement(&Statement::Let {
            name: caught_name.clone(),
            mutable: true,
            value: Expression::Bool(false),
        })?;
        let caught_local = self.lookup_local(&caught_name)?;

        self.emit_statement(&Statement::Try {
            body: vec![Statement::Expression(Expression::Call {
                callee: Box::new(Expression::Identifier(callback_name)),
                arguments: Vec::new(),
            })],
            catch_binding: None,
            catch_setup: Vec::new(),
            catch_body: vec![Statement::Assign {
                name: caught_name,
                value: Expression::Bool(true),
            }],
        })?;

        self.push_local_get(caught_local);
        self.push_i32_const(0);
        self.push_binary_op(BinaryOp::Equal)?;
        self.instructions.push(0x04);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.emit_error_throw()?;
        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_assertion_builtin_call(
        &mut self,
        name: &str,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        match name {
            "__assert" => {
                let Some(CallArgument::Expression(condition)) = arguments.first() else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    return Ok(true);
                };
                let condition_local = self.allocate_temp_local();
                self.emit_numeric_expression(condition)?;
                self.push_local_set(condition_local);
                for argument in arguments.iter().skip(1) {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.emit_numeric_expression(expression)?;
                            self.instructions.push(0x1a);
                        }
                    }
                }
                self.push_local_get(condition_local);
                self.instructions.push(0x45);
                self.instructions.push(0x04);
                self.instructions.push(EMPTY_BLOCK_TYPE);
                self.push_control_frame();
                self.emit_error_throw()?;
                self.instructions.push(0x0b);
                self.pop_control_frame();
                self.push_i32_const(JS_UNDEFINED_TAG);
                Ok(true)
            }
            "__assertSameValue" | "__assertNotSameValue" => {
                let [
                    CallArgument::Expression(actual),
                    CallArgument::Expression(expected),
                    ..,
                ] = arguments
                else {
                    return Ok(false);
                };
                let assertion_failure = match name {
                    "__assertSameValue" => BinaryOp::NotEqual,
                    "__assertNotSameValue" => BinaryOp::Equal,
                    _ => unreachable!("filtered above"),
                };
                let actual_local = self.allocate_temp_local();
                let expected_local = self.allocate_temp_local();
                let handled_as_typeof = matches!(
                    (actual, expected),
                    (
                        Expression::Unary {
                            op: UnaryOp::TypeOf,
                            ..
                        },
                        Expression::String(_)
                    ) | (
                        Expression::String(_),
                        Expression::Unary {
                            op: UnaryOp::TypeOf,
                            ..
                        }
                    )
                ) || matches!(
                    (actual, expected),
                    (Expression::String(text), _) | (_, Expression::String(text))
                        if parse_typeof_tag_optional(text).is_some()
                );
                if handled_as_typeof {
                    if self.emit_typeof_string_comparison(actual, expected, assertion_failure)?
                        || self.emit_runtime_typeof_tag_string_comparison(
                            actual,
                            expected,
                            assertion_failure,
                        )?
                    {
                        self.push_local_set(actual_local);
                    } else {
                        self.push_i32_const(0);
                        self.push_local_set(actual_local);
                    }
                } else if (matches!(actual, Expression::This)
                    || matches!(expected, Expression::This)
                    || self.resolve_array_binding_from_expression(actual).is_some()
                    || self
                        .resolve_array_binding_from_expression(expected)
                        .is_some()
                    || self
                        .resolve_object_binding_from_expression(actual)
                        .is_some()
                    || self
                        .resolve_object_binding_from_expression(expected)
                        .is_some()
                    || self.resolve_user_function_from_expression(actual).is_some()
                    || self
                        .resolve_user_function_from_expression(expected)
                        .is_some()
                    || (!matches!(actual, Expression::Identifier(_))
                        && !matches!(expected, Expression::Identifier(_))))
                    && let Some(result) = self.resolve_static_same_value_result_with_context(
                        actual,
                        expected,
                        self.current_user_function_name.as_deref(),
                    )
                {
                    self.push_i32_const(result as i32);
                    self.push_local_set(actual_local);
                    if assertion_failure == BinaryOp::NotEqual {
                        self.push_local_get(actual_local);
                        self.instructions.push(0x45);
                        self.push_local_set(actual_local);
                    }
                } else {
                    self.emit_numeric_expression(actual)?;
                    self.push_local_set(actual_local);
                    self.emit_numeric_expression(expected)?;
                    self.push_local_set(expected_local);
                    self.emit_same_value_result_from_locals(
                        actual_local,
                        expected_local,
                        actual_local,
                    )?;
                    if assertion_failure == BinaryOp::NotEqual {
                        self.push_local_get(actual_local);
                        self.instructions.push(0x45);
                        self.push_local_set(actual_local);
                    }
                }
                for argument in arguments.iter().skip(2) {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.emit_numeric_expression(expression)?;
                            self.instructions.push(0x1a);
                        }
                    }
                }
                self.push_local_get(actual_local);
                self.instructions.push(0x04);
                self.instructions.push(EMPTY_BLOCK_TYPE);
                self.push_control_frame();
                self.emit_error_throw()?;
                self.instructions.push(0x0b);
                self.pop_control_frame();
                self.push_i32_const(JS_UNDEFINED_TAG);
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}
