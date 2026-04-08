use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn evaluate_bound_snapshot_identifier(
        &self,
        name: &str,
        expression: &Expression,
        bindings: &mut HashMap<String, Expression>,
    ) -> Option<Expression> {
        let resolved_name = self.resolve_bound_snapshot_binding_name(name, bindings);
        if let Some(value) = bindings.get(resolved_name).cloned() {
            if static_expression_matches(&value, expression) {
                return None;
            }
            return Some(value);
        }
        if resolved_name == "undefined" && self.is_unshadowed_builtin_identifier(resolved_name) {
            return Some(Expression::Undefined);
        }
        let identifier = Expression::Identifier(resolved_name.to_string());
        if let Some(array_binding) = self.resolve_array_binding_from_expression(&identifier) {
            return Some(Expression::Array(
                array_binding
                    .values
                    .into_iter()
                    .map(|value| ArrayElement::Expression(value.unwrap_or(Expression::Undefined)))
                    .collect(),
            ));
        }
        if let Some(object_binding) = self.resolve_object_binding_from_expression(&identifier) {
            return Some(object_binding_to_expression(&object_binding));
        }
        if let Some(resolved) = self
            .resolve_bound_alias_expression(&identifier)
            .filter(|resolved| !static_expression_matches(resolved, &identifier))
        {
            return Some(self.materialize_static_expression(&resolved));
        }
        Some(identifier)
    }

    pub(super) fn evaluate_bound_snapshot_this_expression(
        &self,
        expression: &Expression,
        bindings: &mut HashMap<String, Expression>,
        current_function_name: Option<&str>,
    ) -> Option<Expression> {
        match bindings.get("this").cloned() {
            Some(binding) => {
                if matches!(binding, Expression::This)
                    || static_expression_matches(&binding, expression)
                {
                    return None;
                }
                self.evaluate_bound_snapshot_expression(&binding, bindings, current_function_name)
            }
            None => Some(Expression::Undefined),
        }
    }

    pub(super) fn evaluate_bound_snapshot_binary_expression(
        &self,
        op: BinaryOp,
        left: &Expression,
        right: &Expression,
        bindings: &mut HashMap<String, Expression>,
        current_function_name: Option<&str>,
    ) -> Option<Expression> {
        let left =
            self.evaluate_bound_snapshot_expression(left, bindings, current_function_name)?;
        let right =
            self.evaluate_bound_snapshot_expression(right, bindings, current_function_name)?;
        match op {
            BinaryOp::Add => match (&left, &right) {
                (Expression::Number(lhs), Expression::Number(rhs)) => {
                    Some(Expression::Number(lhs + rhs))
                }
                (Expression::String(lhs), Expression::String(rhs)) => {
                    Some(Expression::String(format!("{lhs}{rhs}")))
                }
                _ => None,
            },
            BinaryOp::GreaterThanOrEqual => match (&left, &right) {
                (Expression::Number(lhs), Expression::Number(rhs)) => {
                    Some(Expression::Bool(lhs >= rhs))
                }
                _ => None,
            },
            BinaryOp::LogicalAnd => {
                if self.resolve_static_boolean_expression(&left)? {
                    Some(right)
                } else {
                    Some(left)
                }
            }
            BinaryOp::LogicalOr => {
                if self.resolve_static_boolean_expression(&left)? {
                    Some(left)
                } else {
                    Some(right)
                }
            }
            BinaryOp::NullishCoalescing => {
                if matches!(left, Expression::Null | Expression::Undefined) {
                    Some(right)
                } else {
                    Some(left)
                }
            }
            BinaryOp::Equal
            | BinaryOp::LooseEqual
            | BinaryOp::NotEqual
            | BinaryOp::LooseNotEqual => {
                let equal = match (&left, &right) {
                    (Expression::Bool(lhs), Expression::Bool(rhs)) => lhs == rhs,
                    (Expression::Number(lhs), Expression::Number(rhs)) => lhs == rhs,
                    (Expression::String(lhs), Expression::String(rhs)) => lhs == rhs,
                    (Expression::Null, Expression::Null)
                    | (Expression::Undefined, Expression::Undefined) => true,
                    (Expression::Null, Expression::Undefined)
                    | (Expression::Undefined, Expression::Null)
                        if matches!(op, BinaryOp::LooseEqual | BinaryOp::LooseNotEqual) =>
                    {
                        true
                    }
                    _ => return None,
                };
                Some(Expression::Bool(match op {
                    BinaryOp::Equal | BinaryOp::LooseEqual => equal,
                    BinaryOp::NotEqual | BinaryOp::LooseNotEqual => !equal,
                    _ => unreachable!("filtered above"),
                }))
            }
            _ => None,
        }
    }

    pub(super) fn evaluate_bound_snapshot_member_expression(
        &self,
        object: &Expression,
        property: &Expression,
        bindings: &mut HashMap<String, Expression>,
        current_function_name: Option<&str>,
    ) -> Option<Expression> {
        let object =
            self.evaluate_bound_snapshot_expression(object, bindings, current_function_name)?;
        let property = self.resolve_property_key_expression(property).or_else(|| {
            self.evaluate_bound_snapshot_expression(property, bindings, current_function_name)
        })?;
        if matches!(object, Expression::This)
            && let Expression::String(property_name) = &property
            && let Some(descriptor) =
                self.resolve_top_level_global_property_descriptor_binding(property_name)
            && let Some(value) = descriptor.value
        {
            return Some(value);
        }
        match (object, property) {
            (Expression::Array(elements), Expression::String(name)) if name == "length" => {
                Some(Expression::Number(elements.len() as f64))
            }
            (Expression::Array(elements), Expression::Number(index))
                if index.is_finite() && index.fract() == 0.0 && index >= 0.0 =>
            {
                let index = index as usize;
                match elements.get(index) {
                    Some(ArrayElement::Expression(value)) => Some(value.clone()),
                    Some(ArrayElement::Spread(_)) => None,
                    None => Some(Expression::Undefined),
                }
            }
            (Expression::Object(entries), property) => self
                .resolve_bound_snapshot_object_member_value(
                    &entries,
                    &property,
                    bindings,
                    current_function_name,
                ),
            _ => None,
        }
    }

    pub(super) fn evaluate_bound_snapshot_update_expression(
        &self,
        name: &str,
        op: UpdateOp,
        prefix: bool,
        bindings: &mut HashMap<String, Expression>,
    ) -> Option<Expression> {
        let resolved_name = self
            .resolve_bound_snapshot_binding_name(name, bindings)
            .to_string();
        let current = bindings.get(&resolved_name)?.clone();
        let Expression::Number(current_number) = current else {
            return None;
        };
        let next_number = match op {
            UpdateOp::Increment => current_number + 1.0,
            UpdateOp::Decrement => current_number - 1.0,
        };
        bindings.insert(resolved_name, Expression::Number(next_number));
        Some(if prefix {
            Expression::Number(next_number)
        } else {
            Expression::Number(current_number)
        })
    }
}
