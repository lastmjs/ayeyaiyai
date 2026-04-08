use super::*;

impl<'a> FunctionCompiler<'a> {
    fn sync_static_assign_member_tracking_effect(
        &mut self,
        object: &Expression,
        property: &Expression,
        value: &Expression,
    ) {
        let mut environment = self.snapshot_static_resolution_environment();

        let property = self
            .evaluate_static_expression_with_state(property, &mut environment)
            .unwrap_or_else(|| self.materialize_static_expression(property));
        let value = self
            .evaluate_static_expression_with_state(value, &mut environment)
            .unwrap_or_else(|| self.materialize_static_expression(value));
        let Some(target_name) =
            resolve_stateful_object_binding_name_in_environment(object, &environment).or_else(
                || match object {
                    Expression::Identifier(name) => Some(name.clone()),
                    _ => None,
                },
            )
        else {
            return;
        };
        if !environment.contains_object_binding(&target_name)
            && self
                .resolve_function_binding_from_expression(&Expression::Identifier(
                    target_name.clone(),
                ))
                .is_some()
        {
            environment.set_object_binding(target_name.clone(), empty_object_value_binding());
        }
        let property = self
            .resolve_property_key_expression(&property)
            .unwrap_or(property);
        let Some(binding) = environment.object_binding_mut(&target_name) else {
            return;
        };
        object_binding_set_property(binding, property, value);
        let synced_binding = binding.clone();
        self.state
            .speculation
            .static_semantics
            .set_local_object_binding(&target_name, synced_binding.clone());
        if self.binding_name_is_global(&target_name) {
            self.backend
                .sync_global_object_binding(&target_name, Some(synced_binding));
        }
    }

    pub(in crate::backend::direct_wasm) fn sync_static_statement_tracking_effects(
        &mut self,
        statement: &Statement,
    ) {
        match statement {
            Statement::Declaration { body }
            | Statement::Block { body }
            | Statement::Labeled { body, .. } => {
                for statement in body {
                    self.sync_static_statement_tracking_effects(statement);
                }
            }
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                for statement in then_branch {
                    self.sync_static_statement_tracking_effects(statement);
                }
                for statement in else_branch {
                    self.sync_static_statement_tracking_effects(statement);
                }
            }
            Statement::Var { name, value }
            | Statement::Let { name, value, .. }
            | Statement::Assign { name, value } => {
                self.update_member_function_binding_from_expression(value);
                self.update_object_binding_from_expression(value);
                self.update_capture_slot_binding_from_expression(name, value)
                    .expect("static statement binding sync should succeed");
            }
            Statement::Expression(expression) => {
                self.update_member_function_binding_from_expression(expression);
                self.update_object_binding_from_expression(expression);
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                self.sync_static_assign_member_tracking_effect(object, property, value);
            }
            _ => {}
        }
    }
}
