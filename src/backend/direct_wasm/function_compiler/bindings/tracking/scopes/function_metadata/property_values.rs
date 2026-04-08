use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_user_function_length(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<u32> {
        let resolved_object = self
            .resolve_bound_alias_expression(object)
            .filter(|resolved| !static_expression_matches(resolved, object));
        let materialized_object = self.materialize_static_expression(object);
        let resolved_property = self.resolve_property_key_expression(property).or_else(|| {
            self.resolve_bound_alias_expression(property)
                .filter(|resolved| !static_expression_matches(resolved, property))
        });
        let materialized_property = self.materialize_static_expression(property);

        let object_candidates = [
            Some(object),
            resolved_object.as_ref(),
            (!static_expression_matches(&materialized_object, object))
                .then_some(&materialized_object),
        ];
        let property_candidates = [
            Some(property),
            resolved_property.as_ref(),
            (!static_expression_matches(&materialized_property, property))
                .then_some(&materialized_property),
        ];

        for object_candidate in object_candidates.into_iter().flatten() {
            for property_candidate in property_candidates.into_iter().flatten() {
                if !matches!(property_candidate, Expression::String(property_name) if property_name == "length")
                {
                    continue;
                }
                if self
                    .function_object_has_explicit_own_property(object_candidate, property_candidate)
                {
                    continue;
                }
                match self.resolve_function_binding_from_expression(object_candidate)? {
                    LocalFunctionBinding::User(function_name) => {
                        return self
                            .user_function(&function_name)
                            .map(|user_function| user_function.length);
                    }
                    LocalFunctionBinding::Builtin(function_name) => {
                        return builtin_function_length(&function_name);
                    }
                }
            }
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn runtime_user_function_property_value(
        &self,
        user_function: &UserFunction,
        property_name: &str,
    ) -> Option<Expression> {
        let property = Expression::String(property_name.to_string());
        let function_expression = Expression::Identifier(user_function.name.clone());
        if let Some(object_binding) = self.backend.global_object_binding(&user_function.name)
            && let Some(value) = object_binding_lookup_value(object_binding, &property)
        {
            match value {
                Expression::Identifier(name)
                    if property_name == "name" && name == &user_function.name => {}
                Expression::String(_) | Expression::Number(_) | Expression::Identifier(_) => {
                    return Some(value.clone());
                }
                _ => return None,
            }
        }
        if self.function_object_has_explicit_own_property(&function_expression, &property) {
            return None;
        }
        match property_name {
            "name" => self
                .resolve_user_function_display_name(&user_function.name)
                .map(Expression::String),
            "length" => Some(Expression::Number(user_function.length as f64)),
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_function_name_value(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<String> {
        let resolved_object = self
            .resolve_bound_alias_expression(object)
            .filter(|resolved| !static_expression_matches(resolved, object));
        let materialized_object = self.materialize_static_expression(object);
        let resolved_property = self.resolve_property_key_expression(property).or_else(|| {
            self.resolve_bound_alias_expression(property)
                .filter(|resolved| !static_expression_matches(resolved, property))
        });
        let materialized_property = self.materialize_static_expression(property);

        let object_candidates = [
            Some(object),
            resolved_object.as_ref(),
            (!static_expression_matches(&materialized_object, object))
                .then_some(&materialized_object),
        ];
        let property_candidates = [
            Some(property),
            resolved_property.as_ref(),
            (!static_expression_matches(&materialized_property, property))
                .then_some(&materialized_property),
        ];

        for object_candidate in object_candidates.into_iter().flatten() {
            for property_candidate in property_candidates.into_iter().flatten() {
                if !matches!(property_candidate, Expression::String(property_name) if property_name == "name")
                {
                    continue;
                }
                if self
                    .function_object_has_explicit_own_property(object_candidate, property_candidate)
                {
                    continue;
                }
                match self.resolve_function_binding_from_expression(object_candidate)? {
                    LocalFunctionBinding::User(function_name) => {
                        return self.resolve_user_function_display_name(&function_name);
                    }
                    LocalFunctionBinding::Builtin(function_name) => {
                        return Some(builtin_function_display_name(&function_name).to_string());
                    }
                }
            }
        }
        None
    }
}
