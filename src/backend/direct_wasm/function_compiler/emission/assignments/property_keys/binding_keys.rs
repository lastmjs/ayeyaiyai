use super::*;

impl<'a> FunctionCompiler<'a> {
    fn normalize_member_function_binding_identifier_target(&self, name: &str) -> String {
        self.resolve_registered_function_declaration(name)
            .and_then(|function| function.self_binding.as_ref())
            .or_else(|| {
                self.resolve_registered_function_declaration(name)
                    .and_then(|function| function.top_level_binding.as_ref())
            })
            .cloned()
            .or_else(|| scoped_binding_source_name(name).map(str::to_string))
            .unwrap_or_else(|| name.to_string())
    }

    fn member_function_binding_prototype_target(
        &self,
        expression: &Expression,
    ) -> Option<MemberFunctionBindingTarget> {
        match expression {
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "prototype") =>
            {
                let resolved_object = self
                    .resolve_bound_alias_expression(object)
                    .filter(|resolved| !static_expression_matches(resolved, object))
                    .unwrap_or_else(|| object.as_ref().clone());
                let Expression::Identifier(name) = resolved_object else {
                    return None;
                };
                Some(MemberFunctionBindingTarget::Prototype(
                    self.normalize_member_function_binding_identifier_target(&name),
                ))
            }
            Expression::New { callee, .. } => {
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                Some(MemberFunctionBindingTarget::Prototype(name.clone()))
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn member_function_binding_key(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<MemberFunctionBindingKey> {
        let target = match object {
            Expression::Identifier(name) => self
                .resolve_bound_alias_expression(object)
                .filter(|resolved| !static_expression_matches(resolved, object))
                .and_then(|resolved| {
                    self.member_function_binding_prototype_target(&resolved)
                        .or_else(|| match resolved {
                            Expression::Identifier(resolved_name) => {
                                Some(MemberFunctionBindingTarget::Identifier(
                                    self.normalize_member_function_binding_identifier_target(
                                        &resolved_name,
                                    ),
                                ))
                            }
                            _ => None,
                        })
                })
                .unwrap_or_else(|| {
                    MemberFunctionBindingTarget::Identifier(
                        self.normalize_member_function_binding_identifier_target(name),
                    )
                }),
            _ => self.member_function_binding_prototype_target(object)?,
        };

        let property = self.member_function_binding_property(property)?;

        Some(MemberFunctionBindingKey { target, property })
    }
}
