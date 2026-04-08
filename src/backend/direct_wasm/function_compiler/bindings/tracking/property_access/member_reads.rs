use super::*;

mod getter_calls;
mod runtime_reads;
mod static_reads;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_member_read_without_prelude(
        &mut self,
        object: &Expression,
        property: &Expression,
    ) -> DirectResult<()> {
        let static_array_property = if inline_summary_side_effect_free_expression(property)
            && !self.expression_depends_on_active_loop_assignment(property)
        {
            self.resolve_property_key_expression(property)
                .unwrap_or_else(|| self.materialize_static_expression(property))
        } else {
            property.clone()
        };

        if self.emit_special_member_read_without_prelude(
            object,
            property,
            &static_array_property,
        )? {
            return Ok(());
        }
        if self.emit_member_binding_read_without_prelude(object, property)? {
            return Ok(());
        }
        if self.emit_runtime_or_object_member_read_without_prelude(
            object,
            property,
            &static_array_property,
        )? {
            return Ok(());
        }
        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        Ok(())
    }
}
