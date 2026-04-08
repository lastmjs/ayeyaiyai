use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_assert_throws_call(
        &mut self,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let [
            CallArgument::Expression(expected_error),
            CallArgument::Expression(callback),
            rest @ ..,
        ] = arguments
        else {
            return Ok(false);
        };

        self.emit_numeric_expression(expected_error)?;
        self.state.emission.output.instructions.push(0x1a);
        for argument in rest {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.state.emission.output.instructions.push(0x1a);
                }
            }
        }
        let callback_name =
            self.allocate_named_hidden_local("assert_throws_callback", StaticValueKind::Unknown);
        self.emit_statement(&Statement::Let {
            name: callback_name.clone(),
            mutable: false,
            value: callback.clone(),
        })?;

        let caught_name =
            self.allocate_named_hidden_local("assert_throws_caught", StaticValueKind::Bool);
        self.emit_statement(&Statement::Let {
            name: caught_name.clone(),
            mutable: true,
            value: Expression::Bool(false),
        })?;
        let caught_local = self.lookup_local(&caught_name)?;

        self.emit_statement(&Statement::Try {
            body: vec![Statement::Expression(Expression::Call {
                callee: Box::new(Expression::Identifier(callback_name)),
                arguments: Vec::new(),
            })],
            catch_binding: None,
            catch_setup: Vec::new(),
            catch_body: vec![Statement::Assign {
                name: caught_name,
                value: Expression::Bool(true),
            }],
        })?;

        self.push_local_get(caught_local);
        self.push_i32_const(0);
        self.push_binary_op(BinaryOp::Equal)?;
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

    pub(in crate::backend::direct_wasm) fn emit_assert_throws_statement(
        &mut self,
        expression: &Expression,
    ) -> DirectResult<bool> {
        let Expression::Call { callee, arguments } = expression else {
            return Ok(false);
        };
        let Expression::Identifier(name) = callee.as_ref() else {
            return Ok(false);
        };
        if name != "__ayyAssertThrows" {
            return Ok(false);
        }

        let [
            CallArgument::Expression(expected_error),
            CallArgument::Expression(callback),
            rest @ ..,
        ] = arguments.as_slice()
        else {
            return Ok(false);
        };

        self.emit_numeric_expression(expected_error)?;
        self.state.emission.output.instructions.push(0x1a);
        for argument in rest {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.state.emission.output.instructions.push(0x1a);
                }
            }
        }

        let callback_name =
            self.allocate_named_hidden_local("assert_throws_callback", StaticValueKind::Unknown);
        self.emit_statement(&Statement::Let {
            name: callback_name.clone(),
            mutable: false,
            value: callback.clone(),
        })?;

        let caught_name =
            self.allocate_named_hidden_local("assert_throws_caught", StaticValueKind::Bool);
        self.emit_statement(&Statement::Let {
            name: caught_name.clone(),
            mutable: true,
            value: Expression::Bool(false),
        })?;
        let caught_local = self.lookup_local(&caught_name)?;

        self.emit_statement(&Statement::Try {
            body: vec![Statement::Expression(Expression::Call {
                callee: Box::new(Expression::Identifier(callback_name)),
                arguments: Vec::new(),
            })],
            catch_binding: None,
            catch_setup: Vec::new(),
            catch_body: vec![Statement::Assign {
                name: caught_name,
                value: Expression::Bool(true),
            }],
        })?;

        self.push_local_get(caught_local);
        self.push_i32_const(0);
        self.push_binary_op(BinaryOp::Equal)?;
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
        Ok(true)
    }
}
