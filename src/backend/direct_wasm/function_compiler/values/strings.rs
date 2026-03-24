use super::*;

fn format_static_number(value: f64) -> String {
    if value.is_nan() {
        "NaN".to_string()
    } else if value.is_infinite() {
        if value.is_sign_positive() {
            "Infinity"
        } else {
            "-Infinity"
        }
        .to_string()
    } else if value == 0.0 && value.is_sign_negative() {
        "-0".to_string()
    } else if value.fract() == 0.0 {
        (value as i64).to_string()
    } else {
        value.to_string()
    }
}

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_print(
        &mut self,
        values: &[Expression],
    ) -> DirectResult<()> {
        let (space_ptr, space_len) = self.module.intern_string(b" ".to_vec());
        let (newline_ptr, newline_len) = self.module.intern_string(b"\n".to_vec());

        for (index, value) in values.iter().enumerate() {
            if index > 0 {
                self.push_i32_const(space_ptr as i32);
                self.push_i32_const(space_len as i32);
                self.push_call(WRITE_BYTES_FUNCTION_INDEX);
            }
            self.emit_print_value(value)?;
        }

        self.push_i32_const(newline_ptr as i32);
        self.push_i32_const(newline_len as i32);
        self.push_call(WRITE_BYTES_FUNCTION_INDEX);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_print_value(
        &mut self,
        value: &Expression,
    ) -> DirectResult<()> {
        match value {
            Expression::Number(number) => self.emit_print_string(&format_static_number(*number)),
            Expression::String(text) => {
                let (ptr, len) = self.module.intern_string(text.as_bytes().to_vec());
                self.push_i32_const(ptr as i32);
                self.push_i32_const(len as i32);
                self.push_call(WRITE_BYTES_FUNCTION_INDEX);
                Ok(())
            }
            Expression::Bool(true) => {
                let (ptr, len) = self.module.intern_string(b"true".to_vec());
                self.push_i32_const(ptr as i32);
                self.push_i32_const(len as i32);
                self.push_call(WRITE_BYTES_FUNCTION_INDEX);
                Ok(())
            }
            Expression::Bool(false) => {
                let (ptr, len) = self.module.intern_string(b"false".to_vec());
                self.push_i32_const(ptr as i32);
                self.push_i32_const(len as i32);
                self.push_call(WRITE_BYTES_FUNCTION_INDEX);
                Ok(())
            }
            Expression::Null => {
                let (ptr, len) = self.module.intern_string(b"null".to_vec());
                self.push_i32_const(ptr as i32);
                self.push_i32_const(len as i32);
                self.push_call(WRITE_BYTES_FUNCTION_INDEX);
                Ok(())
            }
            Expression::Undefined => {
                let (ptr, len) = self.module.intern_string(b"undefined".to_vec());
                self.push_i32_const(ptr as i32);
                self.push_i32_const(len as i32);
                self.push_call(WRITE_BYTES_FUNCTION_INDEX);
                Ok(())
            }
            Expression::Unary {
                op: UnaryOp::TypeOf,
                expression,
            } => self.emit_typeof_print(expression),
            Expression::Unary {
                op: UnaryOp::Void, ..
            } => {
                let (ptr, len) = self.module.intern_string(b"undefined".to_vec());
                self.push_i32_const(ptr as i32);
                self.push_i32_const(len as i32);
                self.push_call(WRITE_BYTES_FUNCTION_INDEX);
                Ok(())
            }
            Expression::Unary {
                op: UnaryOp::Delete,
                expression,
            } => {
                match expression.as_ref() {
                    Expression::Identifier(name) => {
                        if self.is_identifier_bound(name) {
                            self.emit_print_string("false")?;
                        } else {
                            self.emit_print_string("true")?;
                        }
                    }
                    Expression::Member { .. }
                    | Expression::SuperMember { .. }
                    | Expression::AssignMember { .. }
                    | Expression::AssignSuperMember { .. }
                    | Expression::This => {
                        self.emit_numeric_expression(expression.as_ref())?;
                        self.instructions.push(0x1a);
                        self.emit_print_string("true")?;
                    }
                    _ => {
                        self.emit_numeric_expression(expression.as_ref())?;
                        self.instructions.push(0x1a);
                        self.emit_print_string("true")?;
                    }
                }
                Ok(())
            }
            _ => {
                if let Some(primitive) = self.resolve_static_primitive_expression_with_context(
                    value,
                    self.current_user_function_name.as_deref(),
                ) && !static_expression_matches(&primitive, value)
                {
                    if !inline_summary_side_effect_free_expression(value) {
                        self.emit_numeric_expression(value)?;
                        self.instructions.push(0x1a);
                    }
                    return self.emit_print_value(&primitive);
                }
                if !matches!(
                    value,
                    Expression::Member { .. } | Expression::SuperMember { .. }
                ) && let Some(number) = self.resolve_static_number_value(value)
                    && (number.is_nan()
                        || !number.is_finite()
                        || number.fract() != 0.0
                        || (number == 0.0 && number.is_sign_negative()))
                {
                    return self.emit_print_value(&Expression::Number(number));
                }
                if let Some(text) = self.resolve_static_string_value(value) {
                    self.emit_print_string(&text)?;
                    return Ok(());
                }
                if self.infer_value_kind(value) == Some(StaticValueKind::Bool) {
                    let bool_local = self.allocate_temp_local();
                    self.emit_numeric_expression(value)?;
                    self.push_local_set(bool_local);
                    self.push_local_get(bool_local);
                    self.instructions.push(0x45);
                    self.instructions.push(0x04);
                    self.instructions.push(EMPTY_BLOCK_TYPE);
                    self.push_control_frame();
                    self.emit_print_string("false")?;
                    self.instructions.push(0x05);
                    self.emit_print_string("true")?;
                    self.instructions.push(0x0b);
                    self.pop_control_frame();
                    return Ok(());
                }
                self.emit_runtime_print_numeric_value(value)
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_print_numeric_value(
        &mut self,
        value: &Expression,
    ) -> DirectResult<()> {
        let value_local = self.allocate_temp_local();
        let handled_local = self.allocate_temp_local();
        self.emit_numeric_expression(value)?;
        self.push_local_set(value_local);
        self.push_i32_const(0);
        self.push_local_set(handled_local);

        for (tag, text) in [(JS_NULL_TAG, "null"), (JS_UNDEFINED_TAG, "undefined")] {
            self.push_local_get(value_local);
            self.push_i32_const(tag);
            self.push_binary_op(BinaryOp::Equal)?;
            self.instructions.push(0x04);
            self.instructions.push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.emit_print_string(text)?;
            self.push_i32_const(1);
            self.push_local_set(handled_local);
            self.instructions.push(0x0b);
            self.pop_control_frame();
        }

        self.push_local_get(handled_local);
        self.instructions.push(0x45);
        self.instructions.push(0x04);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.push_local_get(value_local);
        self.push_i32_const(JS_TYPEOF_NUMBER_TAG);
        self.push_binary_op(BinaryOp::GreaterThanOrEqual)?;
        self.push_local_get(value_local);
        self.push_i32_const(JS_TYPEOF_BIGINT_TAG);
        self.push_binary_op(BinaryOp::LessThanOrEqual)?;
        self.instructions.push(0x71);
        self.instructions.push(0x04);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.emit_typeof_print_from_local(value_local)?;
        self.instructions.push(0x05);
        self.push_local_get(value_local);
        self.push_i32_const(JS_NAN_TAG);
        self.push_binary_op(BinaryOp::Equal)?;
        self.instructions.push(0x04);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.emit_print_string("NaN")?;
        self.instructions.push(0x05);
        self.push_local_get(value_local);
        self.push_call(PRINT_I32_FUNCTION_INDEX);
        self.instructions.push(0x0b);
        self.pop_control_frame();
        self.instructions.push(0x0b);
        self.pop_control_frame();
        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_string_value(
        &self,
        expression: &Expression,
    ) -> Option<String> {
        self.resolve_static_string_value_with_context(
            expression,
            self.current_user_function_name.as_deref(),
        )
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_string_value_with_context(
        &self,
        expression: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<String> {
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self
                .resolve_static_string_value_with_context(&materialized, current_function_name);
        }
        match expression {
            Expression::String(text) => Some(text.clone()),
            Expression::BigInt(value) => Some(parse_static_bigint_literal(value)?.to_string()),
            Expression::Unary {
                op: UnaryOp::Negate,
                ..
            } => Some(self.resolve_static_bigint_value(expression)?.to_string()),
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                let branch = if self.resolve_static_if_condition_value(condition)? {
                    then_expression
                } else {
                    else_expression
                };
                self.resolve_static_string_value_with_context(branch, current_function_name)
            }
            Expression::Identifier(_) => self
                .resolve_bound_alias_expression(expression)
                .filter(|resolved| !static_expression_matches(resolved, expression))
                .or_else(|| {
                    self.resolve_global_value_expression(expression)
                        .filter(|resolved| !static_expression_matches(resolved, expression))
                })
                .and_then(|resolved| {
                    self.resolve_static_string_value_with_context(&resolved, current_function_name)
                }),
            Expression::Binary {
                op: BinaryOp::Add,
                left,
                right,
            } => {
                if let Some(StaticEvalOutcome::Value(value)) = self
                    .resolve_static_addition_outcome_with_context(
                        left,
                        right,
                        current_function_name,
                    )
                {
                    return self
                        .resolve_static_string_value_with_context(&value, current_function_name);
                }
                let left_is_string = self.infer_value_kind(left) == Some(StaticValueKind::String);
                let right_is_string = self.infer_value_kind(right) == Some(StaticValueKind::String);
                if !left_is_string && !right_is_string {
                    return None;
                }
                Some(format!(
                    "{}{}",
                    self.resolve_static_string_concat_value(left, current_function_name)?,
                    self.resolve_static_string_concat_value(right, current_function_name)?
                ))
            }
            Expression::Binary { op, left, right }
                if matches!(
                    op,
                    BinaryOp::LogicalAnd | BinaryOp::LogicalOr | BinaryOp::NullishCoalescing
                ) =>
            {
                let resolved = self.resolve_static_logical_result_expression(*op, left, right)?;
                self.resolve_static_string_value_with_context(&resolved, current_function_name)
            }
            Expression::Member { object, property } => {
                if let Some(function_name) = self.resolve_function_name_value(object, property) {
                    return Some(function_name);
                }
                if let Some(function_binding) = self.resolve_member_getter_binding(object, property)
                {
                    return match function_binding {
                        LocalFunctionBinding::User(function_name) => {
                            let user_function =
                                self.module.user_function_map.get(&function_name)?;
                            let summary = user_function.inline_summary.as_ref()?;
                            if !summary.effects.is_empty() {
                                return None;
                            }
                            let return_value = summary.return_value.as_ref()?;
                            self.resolve_static_string_value_with_context(
                                return_value,
                                Some(function_name.as_str()),
                            )
                        }
                        LocalFunctionBinding::Builtin(_) => None,
                    };
                }
                if let Some(array_binding) = self.resolve_array_binding_from_expression(object) {
                    let index = argument_index_from_expression(property)? as usize;
                    return array_binding
                        .values
                        .get(index)
                        .and_then(|value| value.as_ref())
                        .and_then(|value| {
                            self.resolve_static_string_value_with_context(
                                value,
                                current_function_name,
                            )
                        });
                }
                if let Some(object_binding) = self.resolve_object_binding_from_expression(object) {
                    let materialized_property = self.materialize_static_expression(property);
                    return object_binding_lookup_value(&object_binding, &materialized_property)
                        .and_then(|value| {
                            self.resolve_static_string_value_with_context(
                                value,
                                current_function_name,
                            )
                        });
                }
                if let Expression::String(text) = object.as_ref() {
                    let index = argument_index_from_expression(property)? as usize;
                    return text
                        .chars()
                        .nth(index)
                        .map(|character| character.to_string());
                }
                None
            }
            Expression::Call { callee, arguments } => {
                if arguments.is_empty()
                    && let Expression::Member { object, property } = callee.as_ref()
                    && let Expression::String(property_name) = property.as_ref()
                    && let Some(StaticEvalOutcome::Value(value)) = self
                        .resolve_static_member_call_outcome_with_context(
                            object,
                            property_name,
                            current_function_name,
                        )
                {
                    return self
                        .resolve_static_string_value_with_context(&value, current_function_name);
                }
                if let Some((value, callee_function_name)) = self
                    .resolve_static_call_result_expression_with_context(
                        callee,
                        arguments,
                        current_function_name,
                    )
                {
                    return self.resolve_static_string_value_with_context(
                        &value,
                        callee_function_name.as_deref(),
                    );
                }
                let Expression::Member { object, property } = callee.as_ref() else {
                    return None;
                };
                if !matches!(object.as_ref(), Expression::Identifier(name) if name == "String") {
                    return None;
                }
                if !matches!(property.as_ref(), Expression::String(name) if name == "fromCharCode")
                {
                    return None;
                }
                let [CallArgument::Expression(argument)] = arguments.as_slice() else {
                    return None;
                };
                let Expression::Number(codepoint) = self.resolve_char_code_argument(argument)?
                else {
                    return None;
                };
                char::from_u32(codepoint as u32).map(|character| character.to_string())
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_string_replace_result_with_context(
        &self,
        source: &Expression,
        search_expression: &Expression,
        replacement_expression: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<String> {
        let source_text =
            self.resolve_static_string_value_with_context(source, current_function_name)?;
        let search_text = self
            .resolve_static_string_value_with_context(search_expression, current_function_name)?;
        let Some(match_index) = source_text.find(&search_text) else {
            return Some(source_text);
        };

        let replacement_text = if let Some(text) = self
            .resolve_static_string_value_with_context(replacement_expression, current_function_name)
        {
            text
        } else {
            let binding = self.resolve_function_binding_from_expression_with_context(
                replacement_expression,
                current_function_name,
            )?;
            let LocalFunctionBinding::User(function_name) = binding else {
                return None;
            };
            let user_function = self.module.user_function_map.get(&function_name)?;
            let callback_argument_expressions = vec![
                Expression::String(search_text.clone()),
                Expression::Number(match_index as f64),
                Expression::String(source_text.clone()),
            ];
            let this_binding =
                if self.should_box_sloppy_function_this(user_function, &Expression::Undefined) {
                    Expression::This
                } else {
                    Expression::Undefined
                };
            let replacement_value = self
                .resolve_function_binding_static_return_expression_with_call_frame(
                    &LocalFunctionBinding::User(function_name.clone()),
                    &callback_argument_expressions,
                    &this_binding,
                )?;
            self.resolve_static_string_value_with_context(
                &replacement_value,
                Some(function_name.as_str()),
            )?
        };

        Some(source_text.replacen(&search_text, &replacement_text, 1))
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_string_concat_value(
        &self,
        expression: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<String> {
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.resolve_static_string_concat_value(&materialized, current_function_name);
        }
        if let Expression::Binary {
            op: BinaryOp::Add,
            left,
            right,
        } = expression
            && let Some(StaticEvalOutcome::Value(value)) = self
                .resolve_static_addition_outcome_with_context(left, right, current_function_name)
        {
            return self.resolve_static_string_concat_value(&value, current_function_name);
        }
        match expression {
            Expression::Number(value) => {
                if value.is_nan() {
                    Some("NaN".to_string())
                } else if value.is_infinite() {
                    Some(
                        if value.is_sign_positive() {
                            "Infinity"
                        } else {
                            "-Infinity"
                        }
                        .to_string(),
                    )
                } else if *value == 0.0 && value.is_sign_negative() {
                    Some("-0".to_string())
                } else if value.fract() == 0.0 {
                    Some((*value as i64).to_string())
                } else {
                    Some(value.to_string())
                }
            }
            Expression::BigInt(value) => Some(parse_static_bigint_literal(value)?.to_string()),
            Expression::Unary {
                op: UnaryOp::Negate,
                ..
            } => Some(self.resolve_static_bigint_value(expression)?.to_string()),
            Expression::Bool(value) => Some(if *value { "true" } else { "false" }.to_string()),
            Expression::Null => Some("null".to_string()),
            Expression::Undefined => Some("undefined".to_string()),
            Expression::Identifier(name)
                if name == "undefined" && self.is_unshadowed_builtin_identifier(name) =>
            {
                Some("undefined".to_string())
            }
            Expression::Identifier(name)
                if name == "NaN" && self.is_unshadowed_builtin_identifier(name) =>
            {
                Some("NaN".to_string())
            }
            Expression::Unary {
                op: UnaryOp::TypeOf,
                expression,
            } => self
                .infer_typeof_operand_kind(expression)
                .and_then(StaticValueKind::as_typeof_str)
                .map(str::to_string),
            _ => self.resolve_static_string_value_with_context(expression, current_function_name),
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_typeof_print(
        &mut self,
        expression: &Expression,
    ) -> DirectResult<()> {
        let Some(text) = self
            .infer_typeof_operand_kind(expression)
            .and_then(|kind| kind.as_typeof_str())
        else {
            let type_tag_local = self.allocate_temp_local();
            self.emit_runtime_typeof_tag(expression)?;
            self.push_local_set(type_tag_local);
            return self.emit_typeof_print_from_local(type_tag_local);
        };
        self.emit_print_string(text)
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_typeof_tag(
        &mut self,
        expression: &Expression,
    ) -> DirectResult<()> {
        let value_local = self.allocate_temp_local();
        self.emit_numeric_expression(expression)?;
        self.push_local_set(value_local);
        self.emit_runtime_typeof_tag_from_local(value_local)
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_typeof_tag_from_local(
        &mut self,
        value_local: u32,
    ) -> DirectResult<()> {
        let result_local = self.allocate_temp_local();
        self.push_i32_const(JS_TYPEOF_NUMBER_TAG);
        self.push_local_set(result_local);

        self.emit_runtime_typeof_exact_match(
            value_local,
            result_local,
            JS_NULL_TAG,
            JS_TYPEOF_OBJECT_TAG,
        )?;
        self.emit_runtime_typeof_exact_match(
            value_local,
            result_local,
            JS_UNDEFINED_TAG,
            JS_TYPEOF_UNDEFINED_TAG,
        )?;
        self.emit_runtime_typeof_exact_match(
            value_local,
            result_local,
            JS_TYPEOF_STRING_TAG,
            JS_TYPEOF_STRING_TAG,
        )?;
        self.emit_runtime_typeof_exact_match(
            value_local,
            result_local,
            JS_TYPEOF_BOOLEAN_TAG,
            JS_TYPEOF_BOOLEAN_TAG,
        )?;
        self.emit_runtime_typeof_exact_match(
            value_local,
            result_local,
            JS_TYPEOF_OBJECT_TAG,
            JS_TYPEOF_OBJECT_TAG,
        )?;
        self.emit_runtime_typeof_exact_match(
            value_local,
            result_local,
            JS_TYPEOF_UNDEFINED_TAG,
            JS_TYPEOF_UNDEFINED_TAG,
        )?;
        self.emit_runtime_typeof_exact_match(
            value_local,
            result_local,
            JS_TYPEOF_FUNCTION_TAG,
            JS_TYPEOF_FUNCTION_TAG,
        )?;
        self.emit_runtime_typeof_exact_match(
            value_local,
            result_local,
            JS_BUILTIN_EVAL_VALUE,
            JS_TYPEOF_FUNCTION_TAG,
        )?;
        self.emit_runtime_typeof_exact_match(
            value_local,
            result_local,
            JS_TYPEOF_SYMBOL_TAG,
            JS_TYPEOF_SYMBOL_TAG,
        )?;
        self.emit_runtime_typeof_exact_match(
            value_local,
            result_local,
            JS_TYPEOF_BIGINT_TAG,
            JS_TYPEOF_BIGINT_TAG,
        )?;
        self.emit_runtime_typeof_range_match(
            value_local,
            result_local,
            JS_NATIVE_ERROR_VALUE_BASE,
            JS_NATIVE_ERROR_VALUE_BASE + JS_NATIVE_ERROR_VALUE_LIMIT,
            JS_TYPEOF_OBJECT_TAG,
        )?;
        self.emit_runtime_typeof_range_match(
            value_local,
            result_local,
            JS_USER_FUNCTION_VALUE_BASE,
            JS_USER_FUNCTION_VALUE_BASE + JS_USER_FUNCTION_VALUE_LIMIT,
            JS_TYPEOF_FUNCTION_TAG,
        )?;

        self.push_local_get(result_local);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_typeof_exact_match(
        &mut self,
        value_local: u32,
        result_local: u32,
        match_value: i32,
        result_tag: i32,
    ) -> DirectResult<()> {
        self.push_local_get(value_local);
        self.push_i32_const(match_value);
        self.push_binary_op(BinaryOp::Equal)?;
        self.instructions.push(0x04);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.push_i32_const(result_tag);
        self.push_local_set(result_local);
        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_typeof_range_match(
        &mut self,
        value_local: u32,
        result_local: u32,
        start_inclusive: i32,
        end_exclusive: i32,
        result_tag: i32,
    ) -> DirectResult<()> {
        self.push_local_get(value_local);
        self.push_i32_const(start_inclusive);
        self.push_binary_op(BinaryOp::GreaterThanOrEqual)?;
        self.push_local_get(value_local);
        self.push_i32_const(end_exclusive);
        self.push_binary_op(BinaryOp::LessThan)?;
        self.instructions.push(0x71);
        self.instructions.push(0x04);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.push_i32_const(result_tag);
        self.push_local_set(result_local);
        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_typeof_print_from_local(
        &mut self,
        type_tag_local: u32,
    ) -> DirectResult<()> {
        let done_local = self.allocate_temp_local();
        self.push_i32_const(0);
        self.push_local_set(done_local);

        for (type_tag, text) in [
            (JS_TYPEOF_BOOLEAN_TAG, "boolean"),
            (JS_TYPEOF_STRING_TAG, "string"),
            (JS_TYPEOF_OBJECT_TAG, "object"),
            (JS_TYPEOF_UNDEFINED_TAG, "undefined"),
            (JS_TYPEOF_FUNCTION_TAG, "function"),
            (JS_TYPEOF_SYMBOL_TAG, "symbol"),
            (JS_TYPEOF_BIGINT_TAG, "bigint"),
        ] {
            self.push_local_get(type_tag_local);
            self.push_i32_const(type_tag);
            self.push_binary_op(BinaryOp::Equal)?;
            self.instructions.push(0x04);
            self.instructions.push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.emit_print_string(text)?;
            self.push_i32_const(1);
            self.push_local_set(done_local);
            self.instructions.push(0x0b);
            self.pop_control_frame();
        }

        self.push_local_get(done_local);
        self.instructions.push(0x45);
        self.instructions.push(0x04);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.emit_print_string("number")?;
        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_print_string(
        &mut self,
        text: &str,
    ) -> DirectResult<()> {
        let (ptr, len) = self.module.intern_string(text.as_bytes().to_vec());
        self.push_i32_const(ptr as i32);
        self.push_i32_const(len as i32);
        self.push_call(WRITE_BYTES_FUNCTION_INDEX);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_write_static_string_to_fd(
        &mut self,
        fd: i32,
        text: &str,
    ) -> DirectResult<()> {
        let (ptr, len) = self.module.intern_string(text.as_bytes().to_vec());

        self.push_i32_const(IOVEC_OFFSET as i32);
        self.push_i32_const(ptr as i32);
        self.instructions.push(0x36);
        self.instructions.push(0x02);
        push_u32(&mut self.instructions, 0);

        self.push_i32_const((IOVEC_OFFSET + 4) as i32);
        self.push_i32_const(len as i32);
        self.instructions.push(0x36);
        self.instructions.push(0x02);
        push_u32(&mut self.instructions, 0);

        self.push_i32_const(fd);
        self.push_i32_const(IOVEC_OFFSET as i32);
        self.push_i32_const(1);
        self.push_i32_const(NWRITTEN_OFFSET as i32);
        self.push_call(FD_WRITE_FUNCTION_INDEX);
        self.instructions.push(0x1a);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_stderr_string(
        &mut self,
        text: &str,
    ) -> DirectResult<()> {
        self.emit_write_static_string_to_fd(2, text)
    }

    pub(in crate::backend::direct_wasm) fn emit_uncaught_throw_report_from_locals(
        &mut self,
    ) -> DirectResult<()> {
        let matched_local = self.allocate_temp_local();
        self.push_i32_const(0);
        self.push_local_set(matched_local);

        for name in NATIVE_ERROR_NAMES {
            let Some(value) = native_error_runtime_value(name) else {
                continue;
            };
            self.push_local_get(self.throw_value_local);
            self.push_i32_const(value);
            self.push_binary_op(BinaryOp::Equal)?;
            self.instructions.push(0x04);
            self.instructions.push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.emit_stderr_string(name)?;
            self.emit_stderr_string("\n")?;
            self.push_i32_const(1);
            self.push_local_set(matched_local);
            self.instructions.push(0x0b);
            self.pop_control_frame();
        }

        self.push_local_get(matched_local);
        self.instructions.push(0x45);
        self.push_local_get(self.throw_tag_local);
        self.push_i32_const(0);
        self.push_binary_op(BinaryOp::NotEqual)?;
        self.instructions.push(0x71);
        self.instructions.push(0x04);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.emit_stderr_string("Error\n")?;
        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_static_string_literal(
        &mut self,
        text: &str,
    ) -> DirectResult<()> {
        let (ptr, _) = self.module.intern_string(text.as_bytes().to_vec());
        self.push_i32_const(ptr as i32);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_typeof_string_comparison(
        &mut self,
        left: &Expression,
        right: &Expression,
        op: BinaryOp,
    ) -> DirectResult<bool> {
        let (typeof_expression, type_name) = match (left, right) {
            (
                Expression::Unary {
                    op: UnaryOp::TypeOf,
                    expression,
                },
                Expression::String(text),
            ) => (expression.as_ref(), text.as_str()),
            (
                Expression::String(text),
                Expression::Unary {
                    op: UnaryOp::TypeOf,
                    expression,
                },
            ) => (expression.as_ref(), text.as_str()),
            _ => return Ok(false),
        };

        if let Expression::Member { object, property } = typeof_expression
            && self.is_direct_arguments_object(object)
            && matches!(type_name, "undefined")
            && matches!(
                op,
                BinaryOp::Equal
                    | BinaryOp::LooseEqual
                    | BinaryOp::NotEqual
                    | BinaryOp::LooseNotEqual
            )
            && let Some(index) = argument_index_from_expression(property)
        {
            self.emit_arguments_slot_read(index)?;
            self.push_i32_const(JS_UNDEFINED_TAG);
            let comparison = match op {
                BinaryOp::Equal | BinaryOp::LooseEqual => BinaryOp::Equal,
                BinaryOp::NotEqual | BinaryOp::LooseNotEqual => BinaryOp::NotEqual,
                _ => unreachable!("filtered above"),
            };
            self.push_binary_op(comparison)?;
            return Ok(true);
        }

        let Some(type_tag) = parse_typeof_tag_optional(type_name) else {
            self.emit_numeric_expression(typeof_expression)?;
            self.instructions.push(0x1a);
            self.push_i32_const(match op {
                BinaryOp::Equal | BinaryOp::LooseEqual => 0,
                BinaryOp::NotEqual | BinaryOp::LooseNotEqual => 1,
                _ => return Ok(false),
            });
            return Ok(true);
        };

        self.emit_numeric_expression(&Expression::Unary {
            op: UnaryOp::TypeOf,
            expression: Box::new(typeof_expression.clone()),
        })?;
        self.push_i32_const(type_tag);
        let comparison = match op {
            BinaryOp::Equal | BinaryOp::LooseEqual => BinaryOp::Equal,
            BinaryOp::NotEqual | BinaryOp::LooseNotEqual => BinaryOp::NotEqual,
            _ => return Ok(false),
        };
        self.push_binary_op(comparison)?;
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_typeof_tag_string_comparison(
        &mut self,
        left: &Expression,
        right: &Expression,
        op: BinaryOp,
    ) -> DirectResult<bool> {
        let (value_expression, type_name) = match (left, right) {
            (expression, Expression::String(text)) => (expression, text.as_str()),
            (Expression::String(text), expression) => (expression, text.as_str()),
            _ => return Ok(false),
        };
        let Some(type_tag) = parse_typeof_tag_optional(type_name) else {
            return Ok(false);
        };

        let value_local = self.allocate_temp_local();
        self.emit_numeric_expression(value_expression)?;
        self.push_local_set(value_local);

        self.push_local_get(value_local);
        self.push_i32_const(JS_TYPEOF_NUMBER_TAG);
        self.push_binary_op(BinaryOp::GreaterThanOrEqual)?;
        self.push_local_get(value_local);
        self.push_i32_const(JS_TYPEOF_BIGINT_TAG);
        self.push_binary_op(BinaryOp::LessThanOrEqual)?;
        self.instructions.push(0x71);
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_local_get(value_local);
        self.push_i32_const(type_tag);
        let comparison = match op {
            BinaryOp::Equal | BinaryOp::LooseEqual => BinaryOp::Equal,
            BinaryOp::NotEqual | BinaryOp::LooseNotEqual => BinaryOp::NotEqual,
            _ => return Ok(false),
        };
        self.push_binary_op(comparison)?;
        self.instructions.push(0x05);
        self.push_local_get(value_local);
        self.emit_numeric_expression(&Expression::String(type_name.to_string()))?;
        self.push_binary_op(comparison)?;
        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_hex_quad_string_comparison(
        &mut self,
        left: &Expression,
        right: &Expression,
        op: BinaryOp,
    ) -> DirectResult<bool> {
        let (hex_expression, literal_text) = match (left, right) {
            (expression, Expression::String(text)) => (expression, text.as_str()),
            (Expression::String(text), expression) => (expression, text.as_str()),
            _ => return Ok(false),
        };

        let Some(expected) = parse_fixed_hex_quad(literal_text) else {
            return Ok(false);
        };
        let Some(actual_expression) = self.resolve_hex_quad_numeric_expression(hex_expression)
        else {
            return Ok(false);
        };

        self.emit_numeric_expression(&actual_expression)?;
        self.push_i32_const(expected as i32);
        let comparison = match op {
            BinaryOp::Equal | BinaryOp::LooseEqual => BinaryOp::Equal,
            BinaryOp::NotEqual | BinaryOp::LooseNotEqual => BinaryOp::NotEqual,
            _ => return Ok(false),
        };
        self.push_binary_op(comparison)?;
        Ok(true)
    }
}
