use super::*;

#[path = "runtime_reads/array_reads.rs"]
mod array_reads;
#[path = "runtime_reads/descriptor_reads.rs"]
mod descriptor_reads;
#[path = "runtime_reads/object_reads.rs"]
mod object_reads;
#[path = "runtime_reads/shadow_reads.rs"]
mod shadow_reads;
#[path = "runtime_reads/special_reads.rs"]
mod special_reads;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn emit_runtime_or_object_member_read_without_prelude(
        &mut self,
        object: &Expression,
        property: &Expression,
        static_array_property: &Expression,
    ) -> DirectResult<bool> {
        if self.emit_runtime_descriptor_member_read(object, property)? {
            return Ok(true);
        }
        if self.emit_runtime_array_member_read(object, static_array_property)? {
            return Ok(true);
        }
        if self.emit_runtime_object_shadow_member_read(object, property)? {
            return Ok(true);
        }
        if self.emit_runtime_object_binding_member_read(object, property)? {
            return Ok(true);
        }
        if self.emit_runtime_string_member_read(object, property)? {
            return Ok(true);
        }
        if self.emit_runtime_arguments_member_read(object, property)? {
            return Ok(true);
        }
        if self.emit_runtime_returned_or_function_member_read(object, property)? {
            return Ok(true);
        }
        if self.resolve_array_binding_from_expression(object).is_some() {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(true);
        }
        Ok(false)
    }
}
