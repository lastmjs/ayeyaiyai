use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn emit_runtime_descriptor_member_read(
        &mut self,
        object: &Expression,
        property: &Expression,
    ) -> DirectResult<bool> {
        let Expression::Identifier(name) = object else {
            return Ok(false);
        };
        let resolved_name = self
            .resolve_current_local_binding(name)
            .map(|(resolved_name, _)| resolved_name)
            .unwrap_or_else(|| name.clone());
        let Some(descriptor) = self
            .state
            .speculation
            .static_semantics
            .objects
            .local_descriptor_bindings
            .get(&resolved_name)
        else {
            return Ok(false);
        };
        let Expression::String(property_name) = property else {
            return Ok(false);
        };

        match property_name.as_str() {
            "value" => {
                if let Some(value) = descriptor.value.clone() {
                    self.emit_numeric_expression(&value)?;
                } else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                }
                Ok(true)
            }
            "configurable" => {
                self.push_i32_const(if descriptor.configurable { 1 } else { 0 });
                Ok(true)
            }
            "enumerable" => {
                self.push_i32_const(if descriptor.enumerable { 1 } else { 0 });
                Ok(true)
            }
            "writable" => {
                if let Some(writable) = descriptor.writable {
                    self.push_i32_const(if writable { 1 } else { 0 });
                } else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                }
                Ok(true)
            }
            "get" => {
                if let Some(getter) = descriptor.getter.clone() {
                    self.emit_numeric_expression(&getter)?;
                } else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                }
                Ok(true)
            }
            "set" => {
                if let Some(setter) = descriptor.setter.clone() {
                    self.emit_numeric_expression(&setter)?;
                } else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}
