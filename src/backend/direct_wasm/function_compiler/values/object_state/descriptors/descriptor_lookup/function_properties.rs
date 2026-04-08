use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn function_binding_to_expression(
        binding: &LocalFunctionBinding,
    ) -> Expression {
        match binding {
            LocalFunctionBinding::User(name) | LocalFunctionBinding::Builtin(name) => {
                Expression::Identifier(name.clone())
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_function_property_descriptor_binding(
        &self,
        target: &Expression,
        resolved_target: Option<&Expression>,
        materialized_target: &Expression,
        property_name: &str,
    ) -> Option<PropertyDescriptorBinding> {
        let function_binding = self
            .resolve_function_binding_from_expression(target)
            .or_else(|| {
                resolved_target
                    .and_then(|resolved| self.resolve_function_binding_from_expression(resolved))
            })
            .or_else(|| {
                (!static_expression_matches(materialized_target, target))
                    .then(|| self.resolve_function_binding_from_expression(materialized_target))?
            });
        let binding = function_binding?;
        if property_name == "prototype" {
            let has_prototype = match &binding {
                LocalFunctionBinding::User(function_name)
                | LocalFunctionBinding::Builtin(function_name) => self
                    .resolve_function_prototype_object_binding(function_name)
                    .is_some(),
            };
            if has_prototype {
                return Some(PropertyDescriptorBinding {
                    value: Some(Expression::Member {
                        object: Box::new(target.clone()),
                        property: Box::new(Expression::String("prototype".to_string())),
                    }),
                    configurable: false,
                    enumerable: false,
                    writable: Some(false),
                    getter: None,
                    setter: None,
                    has_get: false,
                    has_set: false,
                });
            }
        }
        let value = match &binding {
            LocalFunctionBinding::User(function_name) => {
                self.user_function(function_name).and_then(|user_function| {
                    self.runtime_user_function_property_value(user_function, property_name)
                })
            }
            LocalFunctionBinding::Builtin(function_name) => match property_name {
                "name" => Some(Expression::String(
                    builtin_function_display_name(function_name).to_string(),
                )),
                "length" => builtin_function_length(function_name)
                    .map(|length| Expression::Number(length as f64)),
                _ => None,
            },
        };
        value.map(|value| PropertyDescriptorBinding {
            value: Some(value),
            configurable: true,
            enumerable: false,
            writable: Some(false),
            getter: None,
            setter: None,
            has_get: false,
            has_set: false,
        })
    }
}
