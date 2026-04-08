use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn evaluate_simple_static_expression_with_bindings(
        &self,
        expression: &Expression,
        bindings: &HashMap<String, Expression>,
    ) -> Option<Expression> {
        match expression {
            Expression::Identifier(name) => Some(
                bindings
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| expression.clone()),
            ),
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::This
            | Expression::NewTarget
            | Expression::Sent => Some(expression.clone()),
            Expression::Binary { op, left, right } => {
                let left = self.evaluate_simple_static_expression_with_bindings(left, bindings)?;
                let right =
                    self.evaluate_simple_static_expression_with_bindings(right, bindings)?;
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
                    BinaryOp::Equal => {
                        Some(Expression::Bool(static_expression_matches(&left, &right)))
                    }
                    BinaryOp::NotEqual => {
                        Some(Expression::Bool(!static_expression_matches(&left, &right)))
                    }
                    _ => None,
                }
            }
            Expression::Object(entries) => {
                let mut evaluated_entries = Vec::new();
                for entry in entries {
                    match entry {
                        ObjectEntry::Data { key, value } => {
                            evaluated_entries.push(ObjectEntry::Data {
                                key: self.evaluate_simple_static_expression_with_bindings(
                                    key, bindings,
                                )?,
                                value: self.evaluate_simple_static_expression_with_bindings(
                                    value, bindings,
                                )?,
                            })
                        }
                        _ => return None,
                    }
                }
                Some(Expression::Object(evaluated_entries))
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn execute_simple_static_user_function_with_bindings(
        &self,
        function_name: &str,
        bindings: &HashMap<String, Expression>,
    ) -> Option<(Expression, HashMap<String, Expression>)> {
        let function = self.resolve_registered_function_declaration(function_name)?;
        let mut local_bindings = bindings.clone();
        for statement in &function.body {
            match statement {
                Statement::Var { name, value } | Statement::Let { name, value, .. } => {
                    let Some(value) = self
                        .evaluate_simple_static_expression_with_bindings(value, &local_bindings)
                    else {
                        return None;
                    };
                    local_bindings.insert(name.clone(), value);
                }
                Statement::Assign { name, value } => {
                    let Some(value) = self
                        .evaluate_simple_static_expression_with_bindings(value, &local_bindings)
                    else {
                        return None;
                    };
                    local_bindings.insert(name.clone(), value);
                }
                Statement::Return(value) => {
                    let Some(value) = self
                        .evaluate_simple_static_expression_with_bindings(value, &local_bindings)
                    else {
                        return None;
                    };
                    return Some((value, local_bindings));
                }
                Statement::Expression(expression) => {
                    if self
                        .evaluate_simple_static_expression_with_bindings(
                            expression,
                            &local_bindings,
                        )
                        .is_none()
                    {
                        return None;
                    }
                }
                Statement::Block { body } if body.is_empty() => {}
                _ => return None,
            }
        }
        None
    }
}
