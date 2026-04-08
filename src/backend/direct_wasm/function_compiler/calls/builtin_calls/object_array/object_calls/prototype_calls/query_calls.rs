use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_object_get_prototype_of_call(
        &mut self,
        callee_object: &Expression,
        callee_property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if !matches!(callee_object, Expression::Identifier(name) if name == "Object") {
            return Ok(false);
        }
        if !matches!(callee_property, Expression::String(name) if name == "getPrototypeOf") {
            return Ok(false);
        }
        let [CallArgument::Expression(target), rest @ ..] = arguments else {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(true);
        };
        self.emit_numeric_expression(target)?;
        self.state.emission.output.instructions.push(0x1a);
        self.discard_call_arguments(rest)?;
        if let Some(prototype) = self.resolve_static_object_prototype_expression(target) {
            self.emit_numeric_expression(&prototype)?;
        } else {
            self.push_i32_const(JS_UNDEFINED_TAG);
        }
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_object_is_extensible_call(
        &mut self,
        callee_object: &Expression,
        callee_property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if !matches!(callee_object, Expression::Identifier(name) if name == "Object") {
            return Ok(false);
        }
        if !matches!(callee_property, Expression::String(name) if name == "isExtensible") {
            return Ok(false);
        }
        let target = match arguments.first() {
            Some(CallArgument::Expression(target)) | Some(CallArgument::Spread(target)) => target,
            None => {
                self.push_i32_const(0);
                return Ok(true);
            }
        };
        self.emit_numeric_expression(target)?;
        self.state.emission.output.instructions.push(0x1a);
        self.discard_call_arguments(&arguments[1..])?;
        self.push_i32_const(
            if self
                .resolve_static_object_prototype_expression(target)
                .is_some()
            {
                1
            } else {
                0
            },
        );
        Ok(true)
    }
}
