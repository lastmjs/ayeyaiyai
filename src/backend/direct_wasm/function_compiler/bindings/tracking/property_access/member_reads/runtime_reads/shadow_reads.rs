use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn emit_runtime_object_shadow_member_read(
        &mut self,
        object: &Expression,
        property: &Expression,
    ) -> DirectResult<bool> {
        let Some(binding) = self.resolve_runtime_object_property_shadow_binding(object, property)
        else {
            return Ok(false);
        };
        let fallback_value = self
            .resolve_object_binding_from_expression(object)
            .and_then(|object_binding| {
                self.resolve_object_binding_property_value(&object_binding, property)
            });
        self.push_global_get(binding.present_index);
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_global_get(binding.value_index);
        self.state.emission.output.instructions.push(0x05);
        if let Some(fallback_value) = fallback_value {
            self.emit_runtime_shadow_fallback_value(&fallback_value)?;
        } else {
            self.push_i32_const(JS_UNDEFINED_TAG);
        }
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(true)
    }
}
