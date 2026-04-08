use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_object_set_prototype_of_call(
        &mut self,
        callee_object: &Expression,
        callee_property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if !matches!(callee_object, Expression::Identifier(name) if name == "Object") {
            return Ok(false);
        }
        if !matches!(callee_property, Expression::String(name) if name == "setPrototypeOf") {
            return Ok(false);
        }

        let [target_argument, prototype_argument, rest @ ..] = arguments else {
            return Ok(false);
        };
        let (
            CallArgument::Expression(target_expression),
            CallArgument::Expression(prototype_expression),
        ) = (target_argument, prototype_argument)
        else {
            return Ok(false);
        };

        let runtime_binding = match target_expression {
            Expression::Identifier(name) => self.global_runtime_prototype_binding(name).cloned(),
            _ => None,
        };
        let materialized_prototype = self.materialize_static_expression(prototype_expression);

        self.emit_numeric_expression(target_expression)?;
        self.state.emission.output.instructions.push(0x1a);
        self.emit_numeric_expression(prototype_expression)?;
        self.state.emission.output.instructions.push(0x1a);
        self.discard_call_arguments(rest)?;

        if let Some(binding) = runtime_binding
            && let Some(global_index) = binding.global_index
            && let Some(variant_index) = binding.variants.iter().position(|candidate| {
                candidate
                    .as_ref()
                    .is_some_and(|candidate| candidate == &materialized_prototype)
            })
        {
            self.push_i32_const(variant_index as i32);
            self.push_global_set(global_index);
        }

        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        Ok(true)
    }
}
