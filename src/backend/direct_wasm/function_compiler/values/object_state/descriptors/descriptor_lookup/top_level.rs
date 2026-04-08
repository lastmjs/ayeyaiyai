use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_top_level_global_property_descriptor_binding(
        &self,
        property_name: &str,
    ) -> Option<PropertyDescriptorBinding> {
        if let Some(state) = self.backend.global_property_descriptor(property_name) {
            return Some(PropertyDescriptorBinding {
                value: state.writable.map(|_| state.value.clone()),
                configurable: state.configurable,
                enumerable: state.enumerable,
                writable: state.writable,
                getter: None,
                setter: None,
                has_get: false,
                has_set: false,
            });
        }
        builtin_identifier_kind(property_name)?;
        Some(PropertyDescriptorBinding {
            value: Some(if property_name == "globalThis" {
                Expression::This
            } else {
                Expression::Member {
                    object: Box::new(Expression::This),
                    property: Box::new(Expression::String(property_name.to_string())),
                }
            }),
            configurable: builtin_identifier_delete_returns_true(property_name),
            enumerable: false,
            writable: Some(!matches!(property_name, "Infinity" | "NaN" | "undefined")),
            getter: None,
            setter: None,
            has_get: false,
            has_set: false,
        })
    }
}
