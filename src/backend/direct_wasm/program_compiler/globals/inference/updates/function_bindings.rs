use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn infer_global_function_binding(
        &self,
        expression: &Expression,
    ) -> Option<LocalFunctionBinding> {
        let materialized = self.materialize_global_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.infer_global_function_binding(&materialized);
        }
        match expression {
            Expression::Identifier(name) => {
                if let Some(binding) = self.global_function_binding(name) {
                    return Some(binding.clone());
                }
                if is_internal_user_function_identifier(name) && self.contains_user_function(name) {
                    Some(LocalFunctionBinding::User(name.clone()))
                } else if builtin_identifier_kind(name) == Some(StaticValueKind::Function) {
                    Some(LocalFunctionBinding::Builtin(name.clone()))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn global_member_function_binding_property(
        &self,
        property: &Expression,
    ) -> Option<MemberFunctionBindingProperty> {
        let materialized = self.materialize_global_expression(property);
        if let Some(property_name) = static_property_name_from_expression(&materialized) {
            return Some(MemberFunctionBindingProperty::String(property_name));
        }
        match &materialized {
            Expression::Member { object, property }
                if matches!(object.as_ref(), Expression::Identifier(name) if name == "Symbol")
                    && matches!(property.as_ref(), Expression::String(_)) =>
            {
                let Expression::String(symbol_name) = property.as_ref() else {
                    unreachable!("filtered above");
                };
                Some(MemberFunctionBindingProperty::Symbol(format!(
                    "Symbol.{symbol_name}"
                )))
            }
            Expression::Call { callee, .. } if matches!(callee.as_ref(), Expression::Identifier(name) if name == "Symbol") => {
                Some(MemberFunctionBindingProperty::SymbolExpression(format!(
                    "{materialized:?}"
                )))
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn global_member_function_binding_key(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<MemberFunctionBindingKey> {
        let target = match object {
            Expression::Identifier(name) => MemberFunctionBindingTarget::Identifier(name.clone()),
            Expression::Member {
                object,
                property: target_property,
            } if matches!(target_property.as_ref(), Expression::String(name) if name == "prototype") =>
            {
                let Expression::Identifier(name) = object.as_ref() else {
                    return None;
                };
                MemberFunctionBindingTarget::Prototype(name.clone())
            }
            Expression::New { callee, .. } => {
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                MemberFunctionBindingTarget::Prototype(name.clone())
            }
            _ => return None,
        };
        let property = self.global_member_function_binding_property(property)?;
        Some(MemberFunctionBindingKey { target, property })
    }
}
