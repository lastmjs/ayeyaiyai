use super::*;

fn js_to_uint32(value: f64) -> u32 {
    if !value.is_finite() || value == 0.0 {
        return 0;
    }
    let truncated = value.trunc();
    let modulo = truncated.rem_euclid(4_294_967_296.0);
    modulo as u32
}

fn js_to_int32(value: f64) -> i32 {
    js_to_uint32(value) as i32
}

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_number_value(
        &self,
        expression: &Expression,
    ) -> Option<f64> {
        if self.expression_depends_on_active_loop_assignment(expression) {
            return None;
        }
        if let Expression::Identifier(name) = expression
            && let Some(resolved) = self
                .resolve_bound_alias_expression(expression)
                .filter(|resolved| !static_expression_matches(resolved, expression))
                .or_else(|| {
                    self.resolve_global_value_expression(expression)
                        .filter(|resolved| !static_expression_matches(resolved, expression))
                })
        {
            let mut referenced_names = HashSet::new();
            collect_referenced_binding_names_from_expression(&resolved, &mut referenced_names);
            if referenced_names.contains(name) {
                return None;
            }
            return self.resolve_static_number_value(&resolved);
        }
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
            if let Expression::Identifier(object_name) = self.materialize_static_expression(object)
                && self.is_unshadowed_builtin_identifier(&object_name)
                && let Expression::String(property_name) =
                    self.materialize_static_expression(property)
                && let Some(value) = builtin_member_number_value(&object_name, &property_name)
            {
                return Some(value);
            }
            if self
                .resolve_user_function_length(object, property)
                .is_some()
            {
                return self
                    .resolve_user_function_length(object, property)
                    .map(f64::from);
            }
            if let Some(bytes_per_element) =
                self.resolve_typed_array_builtin_bytes_per_element(object, property)
            {
                return Some(bytes_per_element as f64);
            }
            if matches!(property.as_ref(), Expression::String(property_name) if property_name == "length")
                && self
                    .resolve_function_binding_from_expression(object)
                    .is_none()
                && self
                    .resolve_member_getter_binding(object, property)
                    .is_none()
                && self
                    .resolve_member_function_binding(object, property)
                    .is_none()
                && self
                    .resolve_member_setter_binding(object, property)
                    .is_none()
                && let Expression::String(text) = self.materialize_static_expression(object)
            {
                return Some(text.encode_utf16().count() as f64);
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
            Expression::Binary {
                op: BinaryOp::BitwiseAnd,
                left,
                right,
            } => Some(
                (js_to_int32(self.resolve_static_number_value(&left)?)
                    & js_to_int32(self.resolve_static_number_value(&right)?))
                    as f64,
            ),
            Expression::Binary {
                op: BinaryOp::BitwiseOr,
                left,
                right,
            } => Some(
                (js_to_int32(self.resolve_static_number_value(&left)?)
                    | js_to_int32(self.resolve_static_number_value(&right)?))
                    as f64,
            ),
            Expression::Binary {
                op: BinaryOp::BitwiseXor,
                left,
                right,
            } => Some(
                (js_to_int32(self.resolve_static_number_value(&left)?)
                    ^ js_to_int32(self.resolve_static_number_value(&right)?))
                    as f64,
            ),
            Expression::Binary {
                op: BinaryOp::LeftShift,
                left,
                right,
            } => Some(
                (js_to_int32(self.resolve_static_number_value(&left)?)
                    << (js_to_uint32(self.resolve_static_number_value(&right)?) & 0x1f))
                    as f64,
            ),
            Expression::Binary {
                op: BinaryOp::RightShift,
                left,
                right,
            } => Some(
                (js_to_int32(self.resolve_static_number_value(&left)?)
                    >> (js_to_uint32(self.resolve_static_number_value(&right)?) & 0x1f))
                    as f64,
            ),
            Expression::Binary {
                op: BinaryOp::UnsignedRightShift,
                left,
                right,
            } => Some(
                (js_to_uint32(self.resolve_static_number_value(&left)?)
                    >> (js_to_uint32(self.resolve_static_number_value(&right)?) & 0x1f))
                    as f64,
            ),
            Expression::Call { callee, arguments } => {
                let (value, callee_function_name) = self
                    .resolve_static_call_result_expression_with_context(
                        &callee,
                        &arguments,
                        self.current_function_name(),
                    )?;
                self.resolve_static_primitive_expression_with_context(
                    &value,
                    callee_function_name
                        .as_deref()
                        .or(self.current_function_name()),
                )
                .and_then(|value| match value {
                    Expression::Number(number) => Some(number),
                    _ => None,
                })
            }
            _ => None,
        }
    }
}
