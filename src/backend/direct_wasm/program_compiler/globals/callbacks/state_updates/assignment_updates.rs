use super::*;

impl DirectWasmCompiler {
    pub(super) fn update_parameter_binding_state_from_member_assignment(
        &self,
        object: &Expression,
        property: &Expression,
        value: &Expression,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) {
        let materialized_property =
            self.materialize_callback_state_expression(property, value_bindings, object_bindings);
        let materialized_value =
            self.materialize_callback_state_expression(value, value_bindings, object_bindings);
        let context = self.static_eval_context();
        let _ = assign_static_member_binding_in_global_maps(
            &context,
            object,
            materialized_property,
            materialized_value,
            &mut HashMap::new(),
            value_bindings,
            object_bindings,
        );
    }

    pub(in crate::backend::direct_wasm) fn update_parameter_binding_state_from_expression(
        &self,
        expression: &Expression,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) {
        match expression {
            Expression::Assign { name, value } => {
                self.update_parameter_binding_state_for_value(
                    name,
                    value,
                    value_bindings,
                    object_bindings,
                );
                return;
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.update_parameter_binding_state_from_member_assignment(
                    object,
                    property,
                    value,
                    value_bindings,
                    object_bindings,
                );
                return;
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.update_parameter_binding_state_from_expression(
                        expression,
                        value_bindings,
                        object_bindings,
                    );
                }
                return;
            }
            _ => {}
        }

        self.update_parameter_binding_state_from_define_property_call(
            expression,
            value_bindings,
            object_bindings,
        );
    }
}
