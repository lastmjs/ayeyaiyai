use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_same_value_result_with_context(
        &self,
        actual: &Expression,
        expected: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<bool> {
        let actual_primitive =
            self.resolve_static_primitive_expression_with_context(actual, current_function_name);
        let expected_primitive =
            self.resolve_static_primitive_expression_with_context(expected, current_function_name);

        if let (Some(actual_primitive), Some(expected_primitive)) =
            (actual_primitive, expected_primitive)
        {
            return match (actual_primitive, expected_primitive) {
                (Expression::Number(actual), Expression::Number(expected)) => {
                    if actual.is_nan() && expected.is_nan() {
                        Some(true)
                    } else if actual == 0.0 && expected == 0.0 {
                        Some(actual.is_sign_negative() == expected.is_sign_negative())
                    } else {
                        Some(actual == expected)
                    }
                }
                (Expression::BigInt(actual), Expression::BigInt(expected)) => Some(
                    parse_static_bigint_literal(&actual)?
                        == parse_static_bigint_literal(&expected)?,
                ),
                (Expression::String(actual), Expression::String(expected)) => {
                    Some(actual == expected)
                }
                (Expression::Bool(actual), Expression::Bool(expected)) => Some(actual == expected),
                (Expression::Null, Expression::Null)
                | (Expression::Undefined, Expression::Undefined) => Some(true),
                _ => None,
            };
        }

        let actual_materialized = self.materialize_static_expression(actual);
        let expected_materialized = self.materialize_static_expression(expected);

        let actual_is_this = matches!(actual_materialized, Expression::This);
        let expected_is_this = matches!(expected_materialized, Expression::This);
        let has_static_reference_identity = |expression: &Expression| {
            self.resolve_object_binding_from_expression(expression)
                .is_some()
                || self
                    .resolve_array_binding_from_expression(expression)
                    .is_some()
                || self
                    .resolve_user_function_from_expression(expression)
                    .is_some()
        };

        if (actual_is_this && !expected_is_this)
            && has_static_reference_identity(&expected_materialized)
        {
            return Some(false);
        }

        if (expected_is_this && !actual_is_this)
            && has_static_reference_identity(&actual_materialized)
        {
            return Some(false);
        }

        if let (Some(actual_symbol), Some(expected_symbol)) = (
            self.resolve_symbol_identity_expression(&actual_materialized),
            self.resolve_symbol_identity_expression(&expected_materialized),
        ) {
            return Some(static_expression_matches(&actual_symbol, &expected_symbol));
        }

        if let (Some(actual_key), Some(expected_key)) = (
            self.resolve_static_reference_identity_key(&actual_materialized),
            self.resolve_static_reference_identity_key(&expected_materialized),
        ) {
            return Some(actual_key == expected_key);
        }

        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_reference_identity_key(
        &self,
        expression: &Expression,
    ) -> Option<String> {
        if matches!(expression, Expression::This) {
            return Some("this".to_string());
        }

        if let Expression::Identifier(name) = expression {
            if let Some(key) = self.reference_identity_key_for_identifier(name) {
                return Some(key);
            }
        }

        if let Some(function) = self.resolve_user_function_from_expression(expression) {
            return Some(format!("user-function:{}", function.name));
        }

        let resolved = self.resolve_bound_alias_expression(expression)?;
        match resolved {
            Expression::This => Some("this".to_string()),
            Expression::Identifier(name) => self.reference_identity_key_for_identifier(&name),
            _ => self
                .resolve_user_function_from_expression(&resolved)
                .map(|function| format!("user-function:{}", function.name)),
        }
    }

    pub(in crate::backend::direct_wasm) fn reference_identity_key_for_identifier(
        &self,
        name: &str,
    ) -> Option<String> {
        if let Some((resolved_name, _)) = self.resolve_current_local_binding(name)
            && (self.local_array_bindings.contains_key(&resolved_name)
                || self.local_object_bindings.contains_key(&resolved_name)
                || self.local_function_bindings.contains_key(&resolved_name))
        {
            return Some(format!("local:{resolved_name}"));
        }
        if self.local_array_bindings.contains_key(name)
            || self.local_object_bindings.contains_key(name)
            || self.local_function_bindings.contains_key(name)
        {
            return Some(format!("local:{name}"));
        }
        if self.module.global_array_bindings.contains_key(name)
            || self.module.global_object_bindings.contains_key(name)
            || self.module.global_function_bindings.contains_key(name)
        {
            return Some(format!("global:{name}"));
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn emit_same_value_result_from_locals(
        &mut self,
        actual_local: u32,
        expected_local: u32,
        result_local: u32,
    ) -> DirectResult<()> {
        self.push_local_get(actual_local);
        self.push_local_get(expected_local);
        self.push_binary_op(BinaryOp::Equal)?;
        self.instructions.push(0x04);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.push_i32_const(1);
        self.push_local_set(result_local);
        self.instructions.push(0x05);
        self.push_local_get(actual_local);
        self.push_i32_const(JS_NAN_TAG);
        self.push_binary_op(BinaryOp::Equal)?;
        self.push_local_get(expected_local);
        self.push_i32_const(JS_NAN_TAG);
        self.push_binary_op(BinaryOp::Equal)?;
        self.instructions.push(0x71);
        self.push_local_set(result_local);
        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_verify_property_call(
        &mut self,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let [object_argument, property_argument, descriptor_argument, ..] = arguments else {
            return Ok(false);
        };
        let (
            CallArgument::Expression(object_expression),
            CallArgument::Expression(property_expression),
            CallArgument::Expression(descriptor_expression),
        ) = (object_argument, property_argument, descriptor_argument)
        else {
            return Ok(false);
        };

        let Some(descriptor) = resolve_property_descriptor_definition(descriptor_expression) else {
            return Ok(false);
        };
        let expected_value = descriptor.value.as_ref().map(|value| {
            let materialized = self.materialize_static_expression(value);
            match materialized {
                Expression::Identifier(name)
                    if name == "undefined" && self.is_unshadowed_builtin_identifier(&name) =>
                {
                    Expression::Undefined
                }
                _ => materialized,
            }
        });
        let expected_writable = descriptor.writable;
        let expected_enumerable = descriptor.enumerable;
        let expected_configurable = descriptor.configurable;
        let matches_value = |actual: &Expression| {
            expected_value
                .as_ref()
                .is_none_or(|expected| expected == actual)
        };
        let matches_bool =
            |actual: bool, expected: Option<bool>| expected.is_none_or(|value| value == actual);
        let matches_missing_bool = |expected: Option<bool>| expected.is_none();

        let direct_arguments = self.is_direct_arguments_object(object_expression);
        let arguments_binding = self.resolve_arguments_binding_from_expression(object_expression);
        let object_binding = self.resolve_object_binding_from_expression(object_expression);

        if direct_arguments && is_symbol_iterator_expression(property_expression) {
            if expected_value
                .as_ref()
                .is_some_and(|value| *value == arguments_symbol_iterator_expression())
                && matches_bool(true, expected_writable)
                && matches_bool(false, expected_enumerable)
                && matches_bool(true, expected_configurable)
            {
                for argument in arguments.iter().skip(3) {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.emit_numeric_expression(expression)?;
                            self.instructions.push(0x1a);
                        }
                    }
                }
                self.push_i32_const(JS_UNDEFINED_TAG);
                return Ok(true);
            }
            return Ok(false);
        }

        let property_name = match property_expression {
            Expression::String(text) => text.clone(),
            Expression::Number(value)
                if value.is_finite() && value.fract() == 0.0 && *value >= 0.0 =>
            {
                (*value as u64).to_string()
            }
            _ => return Ok(false),
        };
        let global_property_descriptor = (self.top_level_function
            && matches!(object_expression, Expression::This))
        .then(|| {
            self.module
                .global_property_descriptors
                .get(&property_name)
                .cloned()
        })
        .flatten();

        if direct_arguments {
            if let Some(index) = canonical_array_index_from_property_name(&property_name) {
                let Some(slot) = self.arguments_slots.get(&index).cloned() else {
                    return Ok(false);
                };
                let matches_descriptor = slot.state.present
                    && matches_bool(slot.state.enumerable, expected_enumerable)
                    && matches_bool(slot.state.configurable, expected_configurable)
                    && if slot.state.is_accessor() {
                        matches_missing_bool(expected_writable) && expected_value.is_none()
                    } else {
                        matches_bool(slot.state.writable, expected_writable)
                    };
                if !matches_descriptor {
                    return Ok(false);
                }
                if let Some(expected_value) = expected_value.as_ref() {
                    let actual_local = self.allocate_temp_local();
                    let expected_local = self.allocate_temp_local();
                    self.emit_arguments_slot_read(index)?;
                    self.push_local_set(actual_local);
                    self.emit_numeric_expression(expected_value)?;
                    self.push_local_set(expected_local);
                    self.push_local_get(actual_local);
                    self.push_local_get(expected_local);
                    self.push_binary_op(BinaryOp::NotEqual)?;
                    self.instructions.push(0x04);
                    self.instructions.push(EMPTY_BLOCK_TYPE);
                    self.push_control_frame();
                    self.emit_error_throw()?;
                    self.instructions.push(0x0b);
                    self.pop_control_frame();
                }
                for argument in arguments.iter().skip(3) {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.emit_numeric_expression(expression)?;
                            self.instructions.push(0x1a);
                        }
                    }
                }
                self.push_i32_const(JS_UNDEFINED_TAG);
                return Ok(true);
            }
        }

        let matches_property = if property_name == "length" {
            if direct_arguments {
                self.current_arguments_length_present
                    && self
                        .current_arguments_length_override
                        .as_ref()
                        .is_none_or(matches_value)
                    && matches_bool(true, expected_writable)
                    && matches_bool(false, expected_enumerable)
                    && matches_bool(true, expected_configurable)
            } else if let Some(arguments_binding) = arguments_binding.as_ref() {
                arguments_binding.length_present
                    && matches_value(&arguments_binding.length_value)
                    && matches_bool(true, expected_writable)
                    && matches_bool(false, expected_enumerable)
                    && matches_bool(true, expected_configurable)
            } else {
                false
            }
        } else if property_name == "callee" {
            let strict = if direct_arguments {
                Some(self.strict_mode)
            } else {
                arguments_binding.as_ref().map(|binding| binding.strict)
            };
            if let Some(strict) = strict {
                if strict {
                    expected_value.is_none()
                        && matches_missing_bool(expected_writable)
                        && matches_bool(false, expected_enumerable)
                        && matches_bool(false, expected_configurable)
                } else {
                    let actual_value = if direct_arguments {
                        self.direct_arguments_callee_expression()
                    } else {
                        arguments_binding
                            .as_ref()
                            .and_then(|binding| binding.callee_value.clone())
                    };
                    let present = if direct_arguments {
                        self.current_arguments_callee_present
                    } else {
                        arguments_binding
                            .as_ref()
                            .is_some_and(|binding| binding.callee_present)
                    };
                    present
                        && actual_value.as_ref().is_none_or(matches_value)
                        && matches_bool(true, expected_writable)
                        && matches_bool(false, expected_enumerable)
                        && matches_bool(true, expected_configurable)
                }
            } else {
                false
            }
        } else if let Some(arguments_binding) = arguments_binding.as_ref() {
            if let Ok(index) = property_name.parse::<usize>() {
                arguments_binding
                    .values
                    .get(index)
                    .is_some_and(matches_value)
                    && matches_bool(true, expected_writable)
                    && matches_bool(true, expected_enumerable)
                    && matches_bool(true, expected_configurable)
            } else {
                false
            }
        } else if let Some(global_property_descriptor) = global_property_descriptor.as_ref() {
            matches_value(&global_property_descriptor.value)
                && match global_property_descriptor.writable {
                    Some(writable) => matches_bool(writable, expected_writable),
                    None => matches_missing_bool(expected_writable),
                }
                && matches_bool(global_property_descriptor.enumerable, expected_enumerable)
                && matches_bool(
                    global_property_descriptor.configurable,
                    expected_configurable,
                )
        } else if let Some(object_binding) = object_binding.as_ref() {
            let property = Expression::String(property_name.clone());
            object_binding_lookup_value(object_binding, &property).is_some_and(matches_value)
                && matches_bool(true, expected_writable)
                && matches_bool(
                    !object_binding
                        .non_enumerable_string_properties
                        .iter()
                        .any(|name| name == &property_name),
                    expected_enumerable,
                )
                && matches_bool(true, expected_configurable)
        } else {
            false
        };

        if !matches_property {
            return Ok(false);
        }

        for argument in arguments.iter().skip(3) {
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

    pub(in crate::backend::direct_wasm) fn resolve_static_number_value(
        &self,
        expression: &Expression,
    ) -> Option<f64> {
        if let Expression::Identifier(name) = expression
            && name == "Infinity"
            && self.is_unshadowed_builtin_identifier(name)
        {
            return Some(f64::INFINITY);
        }
        if let Expression::Member { object, property } = expression
            && matches!(object.as_ref(), Expression::Identifier(name) if name == "Number" && self.is_unshadowed_builtin_identifier(name))
            && let Expression::String(property_name) = property.as_ref()
        {
            return match property_name.as_str() {
                "NaN" => Some(f64::NAN),
                "POSITIVE_INFINITY" => Some(f64::INFINITY),
                "NEGATIVE_INFINITY" => Some(f64::NEG_INFINITY),
                "MAX_VALUE" => Some(f64::MAX),
                "MIN_VALUE" => Some(f64::MIN_POSITIVE),
                _ => None,
            };
        }
        if let Expression::Member { object, property } = expression {
            if let Some(bytes_per_element) =
                self.resolve_typed_array_builtin_bytes_per_element(object, property)
            {
                return Some(bytes_per_element as f64);
            }
        }
        let materialized = self.materialize_static_expression(expression);
        match materialized {
            Expression::Number(value) => Some(value),
            Expression::Bool(value) => Some(if value { 1.0 } else { 0.0 }),
            Expression::Null => Some(0.0),
            Expression::Undefined => Some(f64::NAN),
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                let branch = if self.resolve_static_if_condition_value(&condition)? {
                    &then_expression
                } else {
                    &else_expression
                };
                self.resolve_static_number_value(branch)
            }
            Expression::Identifier(name)
                if name == "undefined" && self.is_unshadowed_builtin_identifier(&name) =>
            {
                Some(f64::NAN)
            }
            Expression::Identifier(name)
                if name == "NaN" && self.is_unshadowed_builtin_identifier(&name) =>
            {
                Some(f64::NAN)
            }
            Expression::Identifier(name)
                if name == "Infinity" && self.is_unshadowed_builtin_identifier(&name) =>
            {
                Some(f64::INFINITY)
            }
            Expression::Unary {
                op: UnaryOp::Plus,
                expression,
            } => self.resolve_static_number_value(&expression),
            Expression::Unary {
                op: UnaryOp::Negate,
                expression,
            } => Some(-self.resolve_static_number_value(&expression)?),
            Expression::Binary {
                op: BinaryOp::Add,
                left,
                right,
            } => Some(
                self.resolve_static_number_value(&left)?
                    + self.resolve_static_number_value(&right)?,
            ),
            Expression::Binary {
                op: BinaryOp::Subtract,
                left,
                right,
            } => Some(
                self.resolve_static_number_value(&left)?
                    - self.resolve_static_number_value(&right)?,
            ),
            Expression::Binary {
                op: BinaryOp::Multiply,
                left,
                right,
            } => Some(
                self.resolve_static_number_value(&left)?
                    * self.resolve_static_number_value(&right)?,
            ),
            Expression::Binary {
                op: BinaryOp::Divide,
                left,
                right,
            } => Some(
                self.resolve_static_number_value(&left)?
                    / self.resolve_static_number_value(&right)?,
            ),
            Expression::Binary {
                op: BinaryOp::Modulo,
                left,
                right,
            } => Some(
                self.resolve_static_number_value(&left)?
                    % self.resolve_static_number_value(&right)?,
            ),
            Expression::Binary {
                op: BinaryOp::Exponentiate,
                left,
                right,
            } => Some(
                self.resolve_static_number_value(&left)?
                    .powf(self.resolve_static_number_value(&right)?),
            ),
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_bigint_value(
        &self,
        expression: &Expression,
    ) -> Option<StaticBigInt> {
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.resolve_static_bigint_value(&materialized);
        }
        match expression {
            Expression::BigInt(value) => parse_static_bigint_literal(value),
            Expression::Unary {
                op: UnaryOp::Negate,
                expression,
            } => Some(-self.resolve_static_bigint_value(expression)?),
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_boolean_expression(
        &self,
        expression: &Expression,
    ) -> Option<bool> {
        let materialized = self.materialize_static_expression(expression);
        match materialized {
            Expression::Bool(value) => Some(value),
            Expression::Null | Expression::Undefined => Some(false),
            Expression::String(text) => Some(!text.is_empty()),
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                let branch = if self.resolve_static_if_condition_value(&condition)? {
                    &then_expression
                } else {
                    &else_expression
                };
                self.resolve_static_boolean_expression(branch)
            }
            Expression::Array(_)
            | Expression::Object(_)
            | Expression::New { .. }
            | Expression::This => Some(true),
            Expression::Identifier(name) => match name.as_str() {
                "undefined" => Some(false),
                "NaN" if self.is_unshadowed_builtin_identifier(name.as_str()) => Some(false),
                _ => None,
            },
            Expression::Unary {
                op: UnaryOp::Not,
                expression,
            } => Some(!self.resolve_static_boolean_expression(&expression)?),
            Expression::Binary { op, left, right } => match op {
                BinaryOp::Equal | BinaryOp::LooseEqual => {
                    if let (Some(left_primitive), Some(right_primitive)) = (
                        self.resolve_static_primitive_expression_with_context(
                            &left,
                            self.current_user_function_name.as_deref(),
                        ),
                        self.resolve_static_primitive_expression_with_context(
                            &right,
                            self.current_user_function_name.as_deref(),
                        ),
                    ) {
                        match (left_primitive, right_primitive) {
                            (Expression::Bool(left), Expression::Bool(right)) => {
                                Some(left == right)
                            }
                            (Expression::Number(left), Expression::Number(right)) => {
                                Some(left == right)
                            }
                            (Expression::String(left), Expression::String(right)) => {
                                Some(left == right)
                            }
                            (Expression::Null, Expression::Null)
                            | (Expression::Undefined, Expression::Undefined) => Some(true),
                            (Expression::Null, Expression::Undefined)
                            | (Expression::Undefined, Expression::Null)
                                if matches!(op, BinaryOp::LooseEqual) =>
                            {
                                Some(true)
                            }
                            _ => None,
                        }
                    } else {
                        None
                    }
                }
                BinaryOp::NotEqual | BinaryOp::LooseNotEqual => {
                    if let (Some(left_primitive), Some(right_primitive)) = (
                        self.resolve_static_primitive_expression_with_context(
                            &left,
                            self.current_user_function_name.as_deref(),
                        ),
                        self.resolve_static_primitive_expression_with_context(
                            &right,
                            self.current_user_function_name.as_deref(),
                        ),
                    ) {
                        match (left_primitive, right_primitive) {
                            (Expression::Bool(left), Expression::Bool(right)) => {
                                Some(left != right)
                            }
                            (Expression::Number(left), Expression::Number(right)) => {
                                Some(left != right)
                            }
                            (Expression::String(left), Expression::String(right)) => {
                                Some(left != right)
                            }
                            (Expression::Null, Expression::Null)
                            | (Expression::Undefined, Expression::Undefined) => Some(false),
                            (Expression::Null, Expression::Undefined)
                            | (Expression::Undefined, Expression::Null)
                                if matches!(op, BinaryOp::LooseNotEqual) =>
                            {
                                Some(false)
                            }
                            _ => None,
                        }
                    } else {
                        None
                    }
                }
                BinaryOp::LessThan
                | BinaryOp::LessThanOrEqual
                | BinaryOp::GreaterThan
                | BinaryOp::GreaterThanOrEqual => {
                    if let (Some(left_number), Some(right_number)) = (
                        self.resolve_static_number_value(&left),
                        self.resolve_static_number_value(&right),
                    ) {
                        Some(match op {
                            BinaryOp::LessThan => left_number < right_number,
                            BinaryOp::LessThanOrEqual => left_number <= right_number,
                            BinaryOp::GreaterThan => left_number > right_number,
                            BinaryOp::GreaterThanOrEqual => left_number >= right_number,
                            _ => unreachable!("filtered above"),
                        })
                    } else {
                        None
                    }
                }
                _ => None,
            },
            Expression::Unary {
                op: UnaryOp::Plus,
                expression,
            }
            | Expression::Unary {
                op: UnaryOp::Negate,
                expression,
            } => {
                let number = self.resolve_static_number_value(&expression)?;
                Some(number != 0.0 && !number.is_nan())
            }
            Expression::Number(value) => Some(value != 0.0 && !value.is_nan()),
            Expression::Call { .. } => self
                .resolve_static_has_own_property_call_result(expression)
                .or_else(|| self.resolve_static_is_nan_call_result(expression))
                .or_else(|| self.resolve_static_object_is_call_result(expression)),
            Expression::Assign { value, .. } => self.resolve_static_boolean_expression(&value),
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_is_nan_call_result(
        &self,
        expression: &Expression,
    ) -> Option<bool> {
        let Expression::Call { callee, arguments } = expression else {
            return None;
        };
        if !matches!(callee.as_ref(), Expression::Identifier(name) if name == "isNaN" && self.is_unshadowed_builtin_identifier(name))
        {
            return None;
        }
        let argument = match arguments.first() {
            Some(CallArgument::Expression(expression)) | Some(CallArgument::Spread(expression)) => {
                expression
            }
            None => &Expression::Undefined,
        };
        let resolved_argument = self
            .resolve_static_primitive_expression_with_context(
                argument,
                self.current_user_function_name.as_deref(),
            )
            .unwrap_or_else(|| argument.clone());
        if let Some(number) = self.resolve_static_number_value(&resolved_argument) {
            Some(number.is_nan())
        } else if let Some(text) = self.resolve_static_string_value(&resolved_argument) {
            Some(parse_string_to_i32(&text).is_err())
        } else if matches!(
            self.infer_value_kind(&resolved_argument),
            Some(
                StaticValueKind::Object
                    | StaticValueKind::Function
                    | StaticValueKind::Symbol
                    | StaticValueKind::BigInt
            )
        ) {
            Some(true)
        } else {
            None
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_has_own_property_call_result(
        &self,
        expression: &Expression,
    ) -> Option<bool> {
        let Expression::Call { callee, arguments } = expression else {
            return None;
        };
        let Expression::Member { object, property } = callee.as_ref() else {
            return None;
        };
        if !matches!(property.as_ref(), Expression::String(name) if name == "hasOwnProperty") {
            return None;
        }
        let [CallArgument::Expression(argument_property)] = arguments.as_slice() else {
            return None;
        };

        if let Some(array_binding) = self.resolve_array_binding_from_expression(object) {
            return Some(
                matches!(argument_property, Expression::String(property_name) if property_name == "length")
                    || argument_index_from_expression(argument_property).is_some_and(|index| {
                        array_binding
                            .values
                            .get(index as usize)
                            .is_some_and(|value| value.is_some())
                    }),
            );
        }

        if self.is_direct_arguments_object(object) {
            return match argument_property {
                Expression::String(property_name) => match property_name.as_str() {
                    "callee" | "length" => Some(self.direct_arguments_has_property(property_name)),
                    _ => canonical_array_index_from_property_name(property_name)
                        .map(|index| self.arguments_slots.contains_key(&index)),
                },
                _ => None,
            };
        }

        if let Some(arguments_binding) = self.resolve_arguments_binding_from_expression(object) {
            return match argument_property {
                Expression::String(property_name) => Some(match property_name.as_str() {
                    "callee" => arguments_binding.callee_present,
                    "length" => arguments_binding.length_present,
                    _ => property_name
                        .parse::<usize>()
                        .ok()
                        .is_some_and(|index| index < arguments_binding.values.len()),
                }),
                _ => None,
            };
        }

        if let Some(user_function) = self.resolve_user_function_from_expression(object) {
            if user_function.is_arrow() {
                return match argument_property {
                    Expression::String(property_name)
                        if property_name == "caller" || property_name == "arguments" =>
                    {
                        Some(false)
                    }
                    _ => None,
                };
            }
        }

        if let Some(object_binding) = self.resolve_object_binding_from_expression(object) {
            return Some(
                self.resolve_object_binding_property_value(&object_binding, argument_property)
                    .is_some(),
            );
        }

        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_object_is_call_result(
        &self,
        expression: &Expression,
    ) -> Option<bool> {
        let Expression::Call { callee, arguments } = expression else {
            return None;
        };
        let Expression::Member { object, property } = callee.as_ref() else {
            return None;
        };
        if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Object" && self.is_unshadowed_builtin_identifier(name))
        {
            return None;
        }
        if !matches!(property.as_ref(), Expression::String(name) if name == "is") {
            return None;
        }
        let [
            CallArgument::Expression(actual),
            CallArgument::Expression(expected),
            ..,
        ] = arguments.as_slice()
        else {
            return None;
        };
        self.resolve_static_same_value_result_with_context(
            actual,
            expected,
            self.current_user_function_name.as_deref(),
        )
    }

    pub(in crate::backend::direct_wasm) fn canonical_object_property_expression(
        &self,
        property: &Expression,
    ) -> Expression {
        let materialized = self.materialize_static_expression(property);
        self.resolve_symbol_identity_expression(&materialized)
            .or_else(|| self.resolve_symbol_identity_expression(property))
            .unwrap_or(materialized)
    }

    pub(in crate::backend::direct_wasm) fn resolve_object_binding_property_value(
        &self,
        object_binding: &ObjectValueBinding,
        property: &Expression,
    ) -> Option<Expression> {
        let canonical_property = self.canonical_object_property_expression(property);
        if let Some(value) = object_binding_lookup_value(object_binding, &canonical_property) {
            return Some(value.clone());
        }

        let requested_symbol = self
            .resolve_symbol_identity_expression(&canonical_property)
            .or_else(|| self.resolve_symbol_identity_expression(property))?;
        object_binding
            .symbol_properties
            .iter()
            .find_map(|(existing_key, value)| {
                let canonical_existing = self
                    .resolve_symbol_identity_expression(existing_key)
                    .unwrap_or_else(|| existing_key.clone());
                (static_expression_matches(&canonical_existing, &requested_symbol)
                    || static_expression_matches(existing_key, &requested_symbol))
                .then(|| value.clone())
            })
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_if_condition_value(
        &self,
        expression: &Expression,
    ) -> Option<bool> {
        if let Expression::Binary { op, left, right } = expression {
            let compare = |lhs: bool, rhs: bool| match op {
                BinaryOp::Equal | BinaryOp::LooseEqual => Some(lhs == rhs),
                BinaryOp::NotEqual | BinaryOp::LooseNotEqual => Some(lhs != rhs),
                _ => None,
            };
            if let Some(lhs) = self.resolve_static_is_nan_call_result(left)
                && let Expression::Bool(rhs) = self.materialize_static_expression(right)
            {
                return compare(lhs, rhs);
            }
            if let Some(rhs) = self.resolve_static_is_nan_call_result(right)
                && let Expression::Bool(lhs) = self.materialize_static_expression(left)
            {
                return compare(lhs, rhs);
            }
        }
        self.resolve_static_boolean_expression(expression)
    }

    pub(in crate::backend::direct_wasm) fn emit_static_string_equality_comparison(
        &mut self,
        left: &Expression,
        right: &Expression,
        op: BinaryOp,
    ) -> DirectResult<bool> {
        let Some(left_text) = self.resolve_static_string_value(left) else {
            return Ok(false);
        };
        let Some(right_text) = self.resolve_static_string_value(right) else {
            return Ok(false);
        };
        let equal = left_text == right_text;
        let result = match op {
            BinaryOp::Equal | BinaryOp::LooseEqual => equal,
            BinaryOp::NotEqual | BinaryOp::LooseNotEqual => !equal,
            _ => return Ok(false),
        };
        self.push_i32_const(if result { 1 } else { 0 });
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn static_expressions_equal(
        &self,
        actual: &Expression,
        expected: &Expression,
    ) -> bool {
        if let (Some(actual_text), Some(expected_text)) = (
            self.resolve_static_string_value(actual),
            self.resolve_static_string_value(expected),
        ) {
            return actual_text == expected_text;
        }

        if let (Some(actual_number), Some(expected_number)) = (
            self.resolve_static_number_value(actual),
            self.resolve_static_number_value(expected),
        ) {
            return actual_number == expected_number;
        }

        self.materialize_static_expression(actual) == self.materialize_static_expression(expected)
    }

    pub(in crate::backend::direct_wasm) fn array_bindings_equal(
        &self,
        actual: &ArrayValueBinding,
        expected: &ArrayValueBinding,
    ) -> bool {
        actual.values.len() == expected.values.len()
            && actual.values.iter().zip(expected.values.iter()).all(
                |(actual_value, expected_value)| match (actual_value, expected_value) {
                    (None, None) => true,
                    (Some(actual_value), Some(expected_value)) => {
                        self.static_expressions_equal(actual_value, expected_value)
                    }
                    _ => false,
                },
            )
    }

    pub(in crate::backend::direct_wasm) fn expand_call_arguments(
        &self,
        arguments: &[CallArgument],
    ) -> Vec<Expression> {
        let mut expanded = Vec::new();
        for argument in arguments {
            match argument {
                CallArgument::Expression(expression) => expanded.push(expression.clone()),
                CallArgument::Spread(expression) => {
                    if let Some(binding) = self.resolve_array_binding_from_expression(expression) {
                        expanded.extend(
                            binding
                                .values
                                .into_iter()
                                .map(|value| value.unwrap_or(Expression::Undefined)),
                        );
                    } else {
                        expanded.push(expression.clone());
                    }
                }
            }
        }
        expanded
    }

    pub(in crate::backend::direct_wasm) fn infer_call_result_kind(
        &self,
        name: &str,
    ) -> Option<StaticValueKind> {
        match name {
            "Number" => Some(StaticValueKind::Number),
            "String" => Some(StaticValueKind::String),
            "Boolean" => Some(StaticValueKind::Bool),
            "isNaN" => Some(StaticValueKind::Bool),
            "Object" | "Array" | "ArrayBuffer" | "Date" | "RegExp" | "Map" | "Set" | "Error"
            | "EvalError" | "RangeError" | "ReferenceError" | "SyntaxError" | "TypeError"
            | "URIError" | "AggregateError" | "Promise" => Some(StaticValueKind::Object),
            "Uint8Array" | "Int8Array" | "Uint16Array" | "Int16Array" | "Uint32Array"
            | "Int32Array" | "Float32Array" | "Float64Array" | "Uint8ClampedArray" => {
                Some(StaticValueKind::Object)
            }
            "BigInt" => Some(StaticValueKind::BigInt),
            "Symbol" => Some(StaticValueKind::Symbol),
            "Function" => Some(StaticValueKind::Function),
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn infer_typeof_operand_kind(
        &self,
        expression: &Expression,
    ) -> Option<StaticValueKind> {
        match expression {
            Expression::Identifier(name)
                if name == "NaN" && self.is_unshadowed_builtin_identifier(name) =>
            {
                Some(StaticValueKind::Number)
            }
            Expression::Identifier(name) => self
                .lookup_identifier_kind(name)
                .or(Some(StaticValueKind::Undefined)),
            _ => self.infer_value_kind(expression),
        }
    }

    pub(in crate::backend::direct_wasm) fn lookup_identifier_kind(
        &self,
        name: &str,
    ) -> Option<StaticValueKind> {
        if name == "arguments" && self.has_arguments_object() {
            return Some(StaticValueKind::Object);
        }
        if parse_test262_realm_identifier(name).is_some()
            || parse_test262_realm_global_identifier(name).is_some()
        {
            return Some(StaticValueKind::Object);
        }
        if let Some((resolved_name, _)) = self.resolve_current_local_binding(name) {
            return Some(
                self.local_kinds
                    .get(&resolved_name)
                    .copied()
                    .unwrap_or(StaticValueKind::Unknown),
            );
        }
        if let Some(hidden_name) = self.resolve_user_function_capture_hidden_name(name)
            && let Some(kind) = self.module.global_kinds.get(&hidden_name)
        {
            return Some(*kind);
        }
        if matches!(
            self.local_function_bindings.get(name),
            Some(LocalFunctionBinding::User(_) | LocalFunctionBinding::Builtin(_))
        ) {
            return Some(StaticValueKind::Function);
        }
        if let Some(kind) = self.module.global_kinds.get(name) {
            return Some(*kind);
        }
        if self.resolve_eval_local_function_hidden_name(name).is_some() {
            return Some(
                self.local_kinds
                    .get(name)
                    .copied()
                    .unwrap_or(StaticValueKind::Unknown),
            );
        }
        if self.module.global_bindings.contains_key(name) {
            return Some(StaticValueKind::Unknown);
        }
        if is_internal_user_function_identifier(name)
            && self.module.user_function_map.contains_key(name)
        {
            return Some(StaticValueKind::Function);
        }
        builtin_identifier_kind(name)
    }

    pub(in crate::backend::direct_wasm) fn infer_value_kind(
        &self,
        expression: &Expression,
    ) -> Option<StaticValueKind> {
        match expression {
            Expression::Number(_) => Some(StaticValueKind::Number),
            Expression::BigInt(_) => Some(StaticValueKind::BigInt),
            Expression::String(_) => Some(StaticValueKind::String),
            Expression::Bool(_) => Some(StaticValueKind::Bool),
            Expression::Null => Some(StaticValueKind::Null),
            Expression::Undefined => Some(StaticValueKind::Undefined),
            Expression::Identifier(name) => Some(
                if name == "undefined" && self.is_unshadowed_builtin_identifier(name) {
                    StaticValueKind::Undefined
                } else if name == "NaN" && self.is_unshadowed_builtin_identifier(name) {
                    StaticValueKind::Number
                } else {
                    self.lookup_identifier_kind(name)
                        .unwrap_or(StaticValueKind::Unknown)
                },
            ),
            Expression::Unary { op, expression } => match op {
                UnaryOp::Void => Some(StaticValueKind::Undefined),
                UnaryOp::Plus => Some(StaticValueKind::Number),
                UnaryOp::Negate => {
                    if self.infer_value_kind(expression) == Some(StaticValueKind::BigInt) {
                        Some(StaticValueKind::BigInt)
                    } else {
                        Some(StaticValueKind::Number)
                    }
                }
                UnaryOp::Not => Some(StaticValueKind::Bool),
                UnaryOp::BitwiseNot => Some(StaticValueKind::Number),
                UnaryOp::TypeOf => Some(StaticValueKind::String),
                UnaryOp::Delete => Some(StaticValueKind::Bool),
            },
            Expression::Binary { op, left, right } => match op {
                BinaryOp::Add => {
                    if let Some(StaticEvalOutcome::Value(value)) = self
                        .resolve_static_addition_outcome_with_context(
                            left,
                            right,
                            self.current_user_function_name.as_deref(),
                        )
                    {
                        return self.infer_value_kind(&value);
                    }
                    Some(StaticValueKind::Number)
                }
                BinaryOp::Subtract
                | BinaryOp::Multiply
                | BinaryOp::Divide
                | BinaryOp::Modulo
                | BinaryOp::Exponentiate
                | BinaryOp::BitwiseAnd
                | BinaryOp::BitwiseOr
                | BinaryOp::BitwiseXor
                | BinaryOp::LeftShift
                | BinaryOp::RightShift
                | BinaryOp::UnsignedRightShift => Some(StaticValueKind::Number),
                BinaryOp::LessThan
                | BinaryOp::LessThanOrEqual
                | BinaryOp::GreaterThan
                | BinaryOp::GreaterThanOrEqual
                | BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::In
                | BinaryOp::InstanceOf
                | BinaryOp::LooseEqual
                | BinaryOp::LooseNotEqual => Some(StaticValueKind::Bool),
                BinaryOp::LogicalAnd | BinaryOp::LogicalOr | BinaryOp::NullishCoalescing => {
                    let left_kind = self.infer_value_kind(left);
                    let right_kind = self.infer_value_kind(right);
                    if left_kind == right_kind {
                        left_kind
                    } else {
                        Some(StaticValueKind::Unknown)
                    }
                }
            },
            Expression::Conditional {
                then_expression,
                else_expression,
                ..
            } => {
                let then_kind = self.infer_value_kind(then_expression);
                let else_kind = self.infer_value_kind(else_expression);
                if then_kind == else_kind {
                    then_kind
                } else {
                    Some(StaticValueKind::Unknown)
                }
            }
            Expression::Assign { value, .. } => self.infer_value_kind(value),
            Expression::AssignMember { value, .. }
            | Expression::AssignSuperMember { value, .. } => self.infer_value_kind(value),
            Expression::Sequence(expressions) => expressions.last().and_then(|last| {
                self.infer_value_kind(last)
                    .or(Some(StaticValueKind::Unknown))
            }),
            Expression::Call { callee, arguments } => {
                if self
                    .resolve_static_has_own_property_call_result(expression)
                    .is_some()
                    || self
                        .resolve_static_object_is_call_result(expression)
                        .is_some()
                {
                    return Some(StaticValueKind::Bool);
                }
                if arguments.is_empty()
                    && let Expression::Member { object, property } = callee.as_ref()
                    && let Expression::String(property_name) = property.as_ref()
                    && let Some(StaticEvalOutcome::Value(value)) = self
                        .resolve_static_member_call_outcome_with_context(
                            object,
                            property_name,
                            self.current_user_function_name.as_deref(),
                        )
                {
                    return self.infer_value_kind(&value);
                }
                if let Some((value, _)) = self.resolve_static_call_result_expression_with_context(
                    callee,
                    arguments,
                    self.current_user_function_name.as_deref(),
                ) {
                    return self.infer_value_kind(&value);
                }
                match callee.as_ref() {
                    Expression::Identifier(name) => self
                        .infer_call_result_kind(name)
                        .or(Some(StaticValueKind::Unknown)),
                    _ => Some(StaticValueKind::Unknown),
                }
            }
            Expression::New { .. } => Some(StaticValueKind::Object),
            Expression::NewTarget => Some(StaticValueKind::Unknown),
            Expression::Member { object, property } => {
                if let Expression::Identifier(name) = object.as_ref()
                    && let Some(descriptor) = self.local_descriptor_bindings.get(name)
                    && let Expression::String(property_name) = property.as_ref()
                {
                    return match property_name.as_str() {
                        "value" => descriptor
                            .value
                            .as_ref()
                            .and_then(|value| self.infer_value_kind(value))
                            .or(Some(StaticValueKind::Undefined)),
                        "configurable" | "enumerable" => Some(StaticValueKind::Bool),
                        "writable" => {
                            if descriptor.writable.is_some() {
                                Some(StaticValueKind::Bool)
                            } else {
                                Some(StaticValueKind::Undefined)
                            }
                        }
                        "get" => {
                            if descriptor.has_get {
                                Some(StaticValueKind::Function)
                            } else {
                                Some(StaticValueKind::Undefined)
                            }
                        }
                        "set" => {
                            if descriptor.has_set {
                                Some(StaticValueKind::Function)
                            } else {
                                Some(StaticValueKind::Undefined)
                            }
                        }
                        _ => Some(StaticValueKind::Unknown),
                    };
                }
                if self.resolve_function_name_value(object, property).is_some() {
                    return Some(StaticValueKind::String);
                }
                if self
                    .resolve_user_function_length(object, property)
                    .is_some()
                {
                    return Some(StaticValueKind::Number);
                }
                if self
                    .resolve_typed_array_builtin_bytes_per_element(object, property)
                    .is_some()
                {
                    return Some(StaticValueKind::Number);
                }
                if let Some(array_binding) = self.resolve_array_binding_from_expression(object) {
                    if matches!(property.as_ref(), Expression::String(name) if name == "length") {
                        return Some(StaticValueKind::Number);
                    }
                    if let Some(index) = argument_index_from_expression(property) {
                        return array_binding
                            .values
                            .get(index as usize)
                            .and_then(|value| value.as_ref())
                            .and_then(|value| self.infer_value_kind(value))
                            .or(Some(StaticValueKind::Undefined));
                    }
                }
                if let Some(object_binding) = self.resolve_object_binding_from_expression(object) {
                    let materialized_property = self.materialize_static_expression(property);
                    return object_binding_lookup_value(&object_binding, &materialized_property)
                        .and_then(|value| self.infer_value_kind(value))
                        .or(Some(StaticValueKind::Undefined));
                }
                if let Expression::String(_) = object.as_ref() {
                    if matches!(property.as_ref(), Expression::String(name) if name == "length") {
                        return Some(StaticValueKind::Number);
                    }
                    if argument_index_from_expression(property).is_some() {
                        return Some(StaticValueKind::String);
                    }
                }
                Some(StaticValueKind::Unknown)
            }
            Expression::SuperMember { .. } => Some(StaticValueKind::Unknown),
            Expression::Update { .. } => Some(StaticValueKind::Number),
            Expression::Array(_) | Expression::Object(_) => Some(StaticValueKind::Object),
            Expression::This => Some(StaticValueKind::Object),
            Expression::Sent
            | Expression::Await(_)
            | Expression::IteratorClose(_)
            | Expression::SuperCall { .. } => Some(StaticValueKind::Undefined),
            Expression::EnumerateKeys(_) | Expression::GetIterator(_) => {
                Some(StaticValueKind::Object)
            }
        }
    }
}
