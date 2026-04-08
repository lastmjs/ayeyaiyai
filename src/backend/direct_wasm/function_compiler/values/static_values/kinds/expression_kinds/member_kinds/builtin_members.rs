use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn infer_builtin_member_kind(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<StaticValueKind> {
        if self.resolve_function_name_value(object, property).is_some() {
            return Some(StaticValueKind::String);
        }
        if self
            .resolve_member_function_binding(object, property)
            .is_some()
        {
            return Some(StaticValueKind::Function);
        }
        if let Expression::Identifier(object_name) = self.materialize_static_expression(object)
            && self.is_unshadowed_builtin_identifier(&object_name)
            && let Expression::String(property_name) = self.materialize_static_expression(property)
            && builtin_member_number_value(&object_name, &property_name).is_some()
        {
            return Some(StaticValueKind::Number);
        }
        if self
            .resolve_user_function_length(object, property)
            .is_some()
        {
            return Some(StaticValueKind::Number);
        }
        if self
            .resolve_typed_array_builtin_bytes_per_element(object, property)
            .is_some()
        {
            return Some(StaticValueKind::Number);
        }
        None
    }
}
