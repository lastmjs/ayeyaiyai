use super::*;

#[path = "assertions/array_compare.rs"]
mod array_compare;
#[path = "assertions/same_value.rs"]
mod same_value;
#[path = "assertions/throws.rs"]
mod throws;
#[path = "assertions/try_scan.rs"]
mod try_scan;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_assertion_builtin_call(
        &mut self,
        name: &str,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        match name {
            "__assert" => {
                let Some(CallArgument::Expression(condition)) = arguments.first() else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    return Ok(true);
                };
                let condition_local = self.allocate_temp_local();
                self.emit_numeric_expression(condition)?;
                self.push_local_set(condition_local);
                for argument in arguments.iter().skip(1) {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.emit_numeric_expression(expression)?;
                            self.state.emission.output.instructions.push(0x1a);
                        }
                    }
                }
                self.push_local_get(condition_local);
                self.state.emission.output.instructions.push(0x45);
                self.state.emission.output.instructions.push(0x04);
                self.state
                    .emission
                    .output
                    .instructions
                    .push(EMPTY_BLOCK_TYPE);
                self.push_control_frame();
                self.emit_error_throw()?;
                self.state.emission.output.instructions.push(0x0b);
                self.pop_control_frame();
                self.push_i32_const(JS_UNDEFINED_TAG);
                Ok(true)
            }
            "__assertSameValue" | "__assertNotSameValue" => {
                self.emit_same_value_assertion(name, arguments)
            }
            _ => Ok(false),
        }
    }
}
