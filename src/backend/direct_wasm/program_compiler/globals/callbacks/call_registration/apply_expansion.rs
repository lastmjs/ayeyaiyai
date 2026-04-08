use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn expand_apply_parameter_call_arguments_from_expression_with_state(
        &self,
        expression: &Expression,
        value_bindings: &HashMap<String, Expression>,
        object_bindings: &HashMap<String, ObjectValueBinding>,
    ) -> Option<Vec<Expression>> {
        let materialized = self
            .materialize_global_expression_with_state(
                expression,
                &HashMap::new(),
                value_bindings,
                object_bindings,
            )
            .unwrap_or_else(|| self.materialize_global_expression(expression));
        match &materialized {
            Expression::Undefined | Expression::Null => Some(Vec::new()),
            Expression::Array(elements) => {
                let mut value_bindings = value_bindings.clone();
                let mut object_bindings = object_bindings.clone();
                let mut values = Vec::new();
                for element in elements {
                    match element {
                        ArrayElement::Expression(expression) => {
                            if let Some(object_binding) = self
                                .infer_global_object_binding_with_state(
                                    expression,
                                    &mut value_bindings,
                                    &mut object_bindings,
                                )
                            {
                                values.push(object_binding_to_expression(&object_binding));
                            } else {
                                values.push(
                                    self.materialize_global_expression_with_state(
                                        expression,
                                        &HashMap::new(),
                                        &value_bindings,
                                        &object_bindings,
                                    )
                                    .unwrap_or_else(|| {
                                        self.materialize_global_expression(expression)
                                    }),
                                );
                            }
                        }
                        ArrayElement::Spread(expression) => {
                            let spread_expression = self
                                .materialize_global_expression_with_state(
                                    expression,
                                    &HashMap::new(),
                                    &value_bindings,
                                    &object_bindings,
                                )
                                .unwrap_or_else(|| self.materialize_global_expression(expression));
                            let array_binding =
                                self.infer_global_array_binding(&spread_expression)?;
                            values.extend(
                                array_binding
                                    .values
                                    .into_iter()
                                    .map(|value| value.unwrap_or(Expression::Undefined)),
                            );
                        }
                    }
                }
                Some(values)
            }
            _ => self.expand_apply_parameter_call_arguments_from_expression(&materialized),
        }
    }

    pub(in crate::backend::direct_wasm) fn expand_apply_parameter_call_arguments_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<Vec<Expression>> {
        let materialized = self.materialize_global_expression(expression);
        match &materialized {
            Expression::Undefined | Expression::Null => Some(Vec::new()),
            Expression::Array(elements) => {
                self.with_cloned_global_binding_state(|value_bindings, object_bindings| {
                    let mut values = Vec::new();
                    for element in elements {
                        match element {
                            ArrayElement::Expression(expression) => {
                                if let Some(object_binding) = self
                                    .infer_global_object_binding_with_state(
                                        expression,
                                        value_bindings,
                                        object_bindings,
                                    )
                                {
                                    values.push(object_binding_to_expression(&object_binding));
                                } else {
                                    values.push(
                                        self.materialize_global_expression_with_state(
                                            expression,
                                            &HashMap::new(),
                                            &value_bindings,
                                            &object_bindings,
                                        )
                                        .unwrap_or_else(
                                            || self.materialize_global_expression(expression),
                                        ),
                                    );
                                }
                            }
                            ArrayElement::Spread(expression) => {
                                let spread_expression = self
                                    .materialize_global_expression_with_state(
                                        expression,
                                        &HashMap::new(),
                                        &value_bindings,
                                        &object_bindings,
                                    )
                                    .unwrap_or_else(|| {
                                        self.materialize_global_expression(expression)
                                    });
                                let array_binding =
                                    self.infer_global_array_binding(&spread_expression)?;
                                values.extend(
                                    array_binding
                                        .values
                                        .into_iter()
                                        .map(|value| value.unwrap_or(Expression::Undefined)),
                                );
                            }
                        }
                    }
                    Some(values)
                })
            }
            _ => {
                if let Some(array_binding) = self.infer_global_array_binding(&materialized) {
                    return Some(
                        array_binding
                            .values
                            .into_iter()
                            .map(|value| value.unwrap_or(Expression::Undefined))
                            .collect(),
                    );
                }
                self.infer_global_arguments_binding(&materialized)
                    .map(|binding| binding.values)
            }
        }
    }
}
