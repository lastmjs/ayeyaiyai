use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn evaluate_bound_snapshot_assign_expression(
        &self,
        name: &str,
        value: &Expression,
        bindings: &mut HashMap<String, Expression>,
        current_function_name: Option<&str>,
    ) -> Option<Expression> {
        let resolved_name = self
            .resolve_bound_snapshot_binding_name(name, bindings)
            .to_string();
        let value =
            self.evaluate_bound_snapshot_expression(value, bindings, current_function_name)?;
        bindings.insert(resolved_name, value.clone());
        Some(value)
    }

    pub(super) fn evaluate_bound_snapshot_assign_member_expression(
        &self,
        object: &Expression,
        property: &Expression,
        value: &Expression,
        bindings: &mut HashMap<String, Expression>,
        current_function_name: Option<&str>,
    ) -> Option<Expression> {
        if let Some(LocalFunctionBinding::User(function_name)) =
            self.resolve_member_setter_binding(object, property)
        {
            let this_binding = match object {
                Expression::Identifier(name) => Expression::Identifier(
                    self.resolve_bound_snapshot_binding_name(name, bindings)
                        .to_string(),
                ),
                Expression::This => bindings
                    .get("this")
                    .cloned()
                    .unwrap_or(Expression::Undefined),
                _ => self.evaluate_bound_snapshot_expression(
                    object,
                    bindings,
                    current_function_name,
                )?,
            };
            let argument =
                self.evaluate_bound_snapshot_expression(value, bindings, current_function_name)?;
            self.apply_bound_snapshot_user_function_call_effects(
                &function_name,
                &[argument.clone()],
                &this_binding,
                bindings,
                current_function_name,
            )?;
            return Some(argument);
        }
        self.apply_bound_snapshot_member_assignment(
            object,
            property,
            value,
            bindings,
            current_function_name,
        )
    }

    pub(super) fn evaluate_bound_snapshot_assign_super_member_expression(
        &self,
        property: &Expression,
        value: &Expression,
        bindings: &mut HashMap<String, Expression>,
        current_function_name: Option<&str>,
    ) -> Option<Expression> {
        let effective_property = self.resolve_property_key_expression(property)?;
        if let Some((_, binding)) =
            self.resolve_super_runtime_prototype_binding_with_context(current_function_name)
        {
            let variants =
                self.resolve_user_super_setter_variants(&binding, &effective_property)?;
            let argument =
                self.evaluate_bound_snapshot_expression(value, bindings, current_function_name)?;
            for (user_function, _) in variants {
                self.apply_bound_snapshot_user_function_call_effects(
                    &user_function.name,
                    &[argument.clone()],
                    &Expression::This,
                    bindings,
                    current_function_name,
                )?;
            }
            return Some(argument);
        }
        let super_base = self.resolve_super_base_expression_with_context(current_function_name)?;
        let LocalFunctionBinding::User(function_name) =
            self.resolve_member_setter_binding(&super_base, &effective_property)?
        else {
            return None;
        };
        let argument =
            self.evaluate_bound_snapshot_expression(value, bindings, current_function_name)?;
        self.apply_bound_snapshot_user_function_call_effects(
            &function_name,
            &[argument.clone()],
            &Expression::This,
            bindings,
            current_function_name,
        )?;
        Some(argument)
    }
}
