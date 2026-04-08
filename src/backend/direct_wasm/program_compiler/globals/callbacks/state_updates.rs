use super::*;

#[path = "state_updates/assignment_updates.rs"]
mod assignment_updates;
#[path = "state_updates/define_property.rs"]
mod define_property;
#[path = "state_updates/statement_updates.rs"]
mod statement_updates;

impl DirectWasmCompiler {
    pub(super) fn materialize_callback_state_expression(
        &self,
        expression: &Expression,
        value_bindings: &HashMap<String, Expression>,
        object_bindings: &HashMap<String, ObjectValueBinding>,
    ) -> Expression {
        self.materialize_global_expression_with_state(
            expression,
            &HashMap::new(),
            value_bindings,
            object_bindings,
        )
        .unwrap_or_else(|| self.materialize_global_expression(expression))
    }

    pub(super) fn update_parameter_binding_state_for_value(
        &self,
        name: &str,
        value: &Expression,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) {
        let materialized_value =
            self.materialize_callback_state_expression(value, value_bindings, object_bindings);
        value_bindings.insert(name.to_string(), materialized_value.clone());
        if let Some(binding) = self.infer_global_object_binding_with_state(
            &materialized_value,
            value_bindings,
            object_bindings,
        ) {
            object_bindings.insert(name.to_string(), binding);
        } else {
            object_bindings.remove(name);
        }
    }
}
