use super::*;

impl<'a> FunctionCompiler<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn emit_async_yield_delegate_done_branch(
        &mut self,
        plan: &AsyncYieldDelegateGeneratorPlan,
        delegate_snapshot_bindings: Option<&HashMap<String, Expression>>,
        runtime_step_result_expression: &Expression,
        step_result_name: &str,
        delegate_completion_name: &str,
        delegate_completion_expression: &Expression,
        promise_value_name: &str,
        promise_done_name: &str,
        property_name: &str,
        index_local: u32,
    ) -> DirectResult<()> {
        self.with_current_user_function_name(Some(plan.function_name.clone()), |compiler| {
            if let Some(completion_expression) = delegate_snapshot_bindings
                .and_then(|snapshot_bindings| snapshot_bindings.get(delegate_completion_name))
                .cloned()
            {
                compiler.emit_statement(&Statement::Assign {
                    name: delegate_completion_name.to_string(),
                    value: completion_expression,
                })?;
            } else if !compiler.emit_async_yield_delegate_step_result_getter_assignment(
                step_result_name,
                runtime_step_result_expression,
                delegate_completion_name,
                "value",
            )? {
                compiler.emit_statement(&Statement::Assign {
                    name: delegate_completion_name.to_string(),
                    value: Expression::Member {
                        object: Box::new(runtime_step_result_expression.clone()),
                        property: Box::new(Expression::String("value".to_string())),
                    },
                })?;
            }
            compiler.push_i32_const(2);
            compiler.push_local_set(index_local);
            match property_name {
                "return" => {
                    compiler.emit_statement(&Statement::Assign {
                        name: promise_value_name.to_string(),
                        value: delegate_completion_expression.clone(),
                    })?;
                    compiler.emit_statement(&Statement::Assign {
                        name: promise_done_name.to_string(),
                        value: Expression::Bool(true),
                    })?;
                }
                "next" | "throw" => {
                    for effect in &plan.completion_effects {
                        compiler.emit_statement(effect)?;
                    }
                    compiler.emit_statement(&Statement::Assign {
                        name: promise_value_name.to_string(),
                        value: plan.completion_value.clone(),
                    })?;
                    compiler.emit_statement(&Statement::Assign {
                        name: promise_done_name.to_string(),
                        value: Expression::Bool(true),
                    })?;
                }
                _ => unreachable!("filtered above"),
            }
            Ok(())
        })
    }

    pub(super) fn emit_async_yield_delegate_not_done_branch(
        &mut self,
        delegate_snapshot_bindings: Option<&HashMap<String, Expression>>,
        runtime_step_result_expression: &Expression,
        step_result_name: &str,
        promise_value_name: &str,
        promise_done_name: &str,
    ) -> DirectResult<()> {
        if let Some(value_expression) = delegate_snapshot_bindings
            .and_then(|snapshot_bindings| snapshot_bindings.get(promise_value_name))
            .cloned()
        {
            self.emit_statement(&Statement::Assign {
                name: promise_value_name.to_string(),
                value: value_expression,
            })?;
        } else if !self.emit_async_yield_delegate_step_result_getter_assignment(
            step_result_name,
            runtime_step_result_expression,
            promise_value_name,
            "value",
        )? {
            self.emit_statement(&Statement::Assign {
                name: promise_value_name.to_string(),
                value: Expression::Member {
                    object: Box::new(runtime_step_result_expression.clone()),
                    property: Box::new(Expression::String("value".to_string())),
                },
            })?;
        }
        self.emit_statement(&Statement::Assign {
            name: promise_done_name.to_string(),
            value: Expression::Bool(false),
        })?;
        Ok(())
    }
}
