use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_call_descriptor_binding(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Option<PropertyDescriptorBinding> {
        let resolved_callee = self
            .resolve_bound_alias_expression(callee)
            .unwrap_or_else(|| self.materialize_static_expression(callee));
        let Expression::Member { object, property } = &resolved_callee else {
            return None;
        };
        if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Object") {
            return None;
        }
        if !matches!(property.as_ref(), Expression::String(name) if name == "getOwnPropertyDescriptor")
        {
            return None;
        }
        let [
            CallArgument::Expression(target),
            CallArgument::Expression(property_name),
            ..,
        ] = arguments
        else {
            return None;
        };
        let property = self
            .resolve_property_key_expression(property_name)
            .unwrap_or_else(|| self.materialize_static_expression(property_name));
        let string_property_name = static_property_name_from_expression(&property);
        if let Some(descriptor) =
            self.resolve_arguments_descriptor_binding(target, string_property_name.as_deref())
        {
            return Some(descriptor);
        }
        if self.state.speculation.execution_context.top_level_function
            && matches!(target, Expression::This)
            && let Some(property_name) = string_property_name.as_deref()
        {
            return self.resolve_top_level_global_property_descriptor_binding(property_name);
        }
        let resolved_target = self
            .resolve_bound_alias_expression(target)
            .filter(|resolved| !static_expression_matches(resolved, target));
        let materialized_target = self.materialize_static_expression(target);
        if let Some(property_name) = string_property_name.as_deref()
            && let Some(descriptor) = self.resolve_function_property_descriptor_binding(
                target,
                resolved_target.as_ref(),
                &materialized_target,
                property_name,
            )
        {
            return Some(descriptor);
        }
        self.resolve_object_property_descriptor_binding(
            target,
            resolved_target.as_ref(),
            &materialized_target,
            &property,
            string_property_name.as_deref(),
        )
    }
}
