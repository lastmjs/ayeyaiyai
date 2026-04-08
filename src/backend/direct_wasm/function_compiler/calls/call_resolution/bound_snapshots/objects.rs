use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_bound_snapshot_object_member_value(
        &self,
        entries: &[ObjectEntry],
        property: &Expression,
        bindings: &mut HashMap<String, Expression>,
        current_function_name: Option<&str>,
    ) -> Option<Expression> {
        match self.resolve_bound_snapshot_object_member_outcome(
            entries,
            property,
            bindings,
            current_function_name,
        )? {
            StaticEvalOutcome::Value(value) => Some(value),
            StaticEvalOutcome::Throw(_) => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_bound_snapshot_object_member_outcome(
        &self,
        entries: &[ObjectEntry],
        property: &Expression,
        bindings: &mut HashMap<String, Expression>,
        current_function_name: Option<&str>,
    ) -> Option<StaticEvalOutcome> {
        for entry in entries.iter().rev() {
            match entry {
                ObjectEntry::Data { key, value } => {
                    let key = self.resolve_property_key_expression(key).or_else(|| {
                        self.evaluate_bound_snapshot_expression(
                            key,
                            bindings,
                            current_function_name,
                        )
                    })?;
                    if static_expression_matches(&key, property) {
                        return self
                            .evaluate_bound_snapshot_expression(
                                value,
                                bindings,
                                current_function_name,
                            )
                            .map(StaticEvalOutcome::Value);
                    }
                }
                ObjectEntry::Getter { key, getter } => {
                    let key = self.resolve_property_key_expression(key).or_else(|| {
                        self.evaluate_bound_snapshot_expression(
                            key,
                            bindings,
                            current_function_name,
                        )
                    })?;
                    if !static_expression_matches(&key, property) {
                        continue;
                    }
                    let getter_binding = self
                        .resolve_function_binding_from_expression_with_context(
                            getter,
                            current_function_name,
                        )?;
                    let (outcome, updated_bindings) = self
                        .resolve_bound_snapshot_function_outcome_with_arguments_and_this(
                            &getter_binding,
                            bindings,
                            &[],
                            &Expression::Object(entries.to_vec()),
                        )?;
                    *bindings = updated_bindings;
                    return Some(outcome);
                }
                ObjectEntry::Setter { key, .. } => {
                    let key = self.resolve_property_key_expression(key).or_else(|| {
                        self.evaluate_bound_snapshot_expression(
                            key,
                            bindings,
                            current_function_name,
                        )
                    })?;
                    if static_expression_matches(&key, property) {
                        return Some(StaticEvalOutcome::Value(Expression::Undefined));
                    }
                }
                ObjectEntry::Spread(expression) => {
                    let spread = self.evaluate_bound_snapshot_expression(
                        expression,
                        bindings,
                        current_function_name,
                    )?;
                    let Expression::Object(spread_entries) = spread else {
                        return None;
                    };
                    if let Some(value) = self.resolve_bound_snapshot_object_member_outcome(
                        &spread_entries,
                        property,
                        bindings,
                        current_function_name,
                    ) {
                        return Some(value);
                    }
                }
            }
        }
        Some(StaticEvalOutcome::Value(Expression::Undefined))
    }
}
