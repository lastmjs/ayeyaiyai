use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn compile(
        self,
        statements: &[Statement],
    ) -> DirectResult<CompiledFunction> {
        let mut compiler = self;
        compiler.compile_in_current_global_scope(statements)
    }

    fn compile_in_current_global_scope(
        &mut self,
        statements: &[Statement],
    ) -> DirectResult<CompiledFunction> {
        self.bindings_domain().register_statements(statements)?;
        self.push_global_get(THROW_TAG_GLOBAL_INDEX);
        self.push_local_set(self.state.runtime.throws.throw_tag_local);
        self.push_global_get(THROW_VALUE_GLOBAL_INDEX);
        self.push_local_set(self.state.runtime.throws.throw_value_local);
        if let Some(parameter_scope_arguments_local) =
            self.state.parameters.parameter_scope_arguments_local
        {
            self.push_i32_const(JS_UNDEFINED_TAG);
            self.push_local_set(parameter_scope_arguments_local);
        }
        if let Some(local_index) = self.state.speculation.execution_context.self_binding_local
            && let Some(runtime_value) = self
                .state
                .speculation
                .execution_context
                .self_binding_runtime_value
        {
            self.push_i32_const(runtime_value);
            self.push_local_set(local_index);
        }
        let parameter_initialized_locals = self
            .state
            .parameters
            .parameter_initialized_locals
            .values()
            .copied()
            .collect::<Vec<_>>();
        for initialized_local in parameter_initialized_locals {
            self.push_i32_const(0);
            self.push_local_set(initialized_local);
        }
        self.initialize_arguments_object(statements)?;
        self.initialize_parameter_defaults()?;
        self.control_flow_domain().emit_direct_scope(statements)?;

        self.exception_domain().clear_throw_state();
        if self.state.runtime.behavior.allow_return {
            if self.current_function_is_derived_constructor() {
                let this_local = self.allocate_temp_local();
                self.push_global_get(CURRENT_THIS_GLOBAL_INDEX);
                self.push_local_tee(this_local);
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_binary_op(BinaryOp::Equal)?;
                self.state.emission.output.instructions.push(0x04);
                self.state.emission.output.instructions.push(I32_TYPE);
                self.push_control_frame();
                self.emit_named_error_throw("ReferenceError")?;
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.state.emission.output.instructions.push(0x05);
                self.push_local_get(this_local);
                self.state.emission.output.instructions.push(0x0b);
                self.pop_control_frame();
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
        }

        let instructions = std::mem::take(&mut self.state.emission.output.instructions);
        Ok(CompiledFunction {
            local_count: self.state.runtime.locals.next_local_index
                - self.state.parameters.param_count,
            instructions,
        })
    }
}
