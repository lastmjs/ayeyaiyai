use super::*;

mod arguments_objects;
mod named_objects;
mod setter_calls;
mod super_members;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_assign_member_expression(
        &mut self,
        object: &Expression,
        property: &Expression,
        value: &Expression,
    ) -> DirectResult<()> {
        if self.emit_arguments_or_restricted_member_assignment(object, property, value)? {
            return Ok(());
        }

        if let Expression::Identifier(name) = object
            && matches!(property, Expression::String(property_name) if property_name == "prototype")
        {
            self.update_prototype_object_binding(name, value);
        }

        if self.emit_setter_member_assignment(object, property, value)? {
            return Ok(());
        }

        if self.emit_named_object_member_assignment(object, property, value)? {
            return Ok(());
        }

        self.emit_numeric_expression(object)?;
        self.state.emission.output.instructions.push(0x1a);
        self.emit_numeric_expression(property)?;
        self.state.emission.output.instructions.push(0x1a);
        self.emit_numeric_expression(value)?;
        self.state.emission.output.instructions.push(0x1a);
        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        Ok(())
    }
}
