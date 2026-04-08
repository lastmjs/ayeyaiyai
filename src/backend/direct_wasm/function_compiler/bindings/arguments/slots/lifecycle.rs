use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn initialize_arguments_object(
        &mut self,
        statements: &[Statement],
    ) -> DirectResult<()> {
        let Some(actual_argument_count_local) = self.state.parameters.actual_argument_count_local
        else {
            return Ok(());
        };

        let arguments_usage = collect_arguments_usage_from_statements(statements);
        for index in arguments_usage.indexed_slots {
            let source_param_local = if index < self.state.parameters.visible_param_count {
                Some(index)
            } else {
                self.state
                    .parameters
                    .extra_argument_param_locals
                    .get(&index)
                    .copied()
            };
            let visible_param = index < self.state.parameters.visible_param_count;
            let mapped_argument = self.state.parameters.mapped_arguments && visible_param;
            let value_local = self.allocate_temp_local();
            let present_local = self.allocate_temp_local();
            let mapped_local = if visible_param {
                Some(self.allocate_temp_local())
            } else {
                None
            };

            if let Some(source_param_local) = source_param_local {
                self.push_local_get(source_param_local);
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            self.push_local_set(value_local);

            self.push_local_get(actual_argument_count_local);
            self.push_i32_const(index as i32);
            self.push_binary_op(BinaryOp::GreaterThan)?;
            self.push_local_set(present_local);

            if let Some(mapped_local) = mapped_local {
                if self.state.parameters.mapped_arguments {
                    self.push_local_get(present_local);
                } else {
                    self.push_i32_const(0);
                }
                self.push_local_set(mapped_local);
            }

            self.state.parameters.arguments_slots.insert(
                index,
                ArgumentsSlot {
                    value_local,
                    present_local,
                    mapped_local,
                    source_param_local: visible_param.then_some(index),
                    state: ArgumentsIndexedPropertyState::data(visible_param, mapped_argument),
                },
            );
        }

        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn ensure_arguments_slot(
        &mut self,
        index: u32,
    ) -> DirectResult<()> {
        if self.state.parameters.arguments_slots.contains_key(&index) {
            return Ok(());
        }

        let visible_param = index < self.state.parameters.visible_param_count;
        let mapped_argument = self.state.parameters.mapped_arguments && visible_param;
        let value_local = self.allocate_temp_local();
        let present_local = self.allocate_temp_local();
        let mapped_local = if visible_param {
            Some(self.allocate_temp_local())
        } else {
            None
        };

        if let Some(source_param_local) = if visible_param {
            Some(index)
        } else {
            self.state
                .parameters
                .extra_argument_param_locals
                .get(&index)
                .copied()
        } {
            self.push_local_get(source_param_local);
        } else {
            self.push_i32_const(JS_UNDEFINED_TAG);
        }
        self.push_local_set(value_local);

        if let Some(actual_argument_count_local) = self.state.parameters.actual_argument_count_local
        {
            self.push_local_get(actual_argument_count_local);
            self.push_i32_const(index as i32);
            self.push_binary_op(BinaryOp::GreaterThan)?;
        } else {
            self.push_i32_const(0);
        }
        self.push_local_set(present_local);

        if let Some(mapped_local) = mapped_local {
            if self.state.parameters.mapped_arguments {
                self.push_local_get(present_local);
            } else {
                self.push_i32_const(0);
            }
            self.push_local_set(mapped_local);
        }

        self.state.parameters.arguments_slots.insert(
            index,
            ArgumentsSlot {
                value_local,
                present_local,
                mapped_local,
                source_param_local: visible_param.then_some(index),
                state: ArgumentsIndexedPropertyState::data(visible_param, mapped_argument),
            },
        );
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn has_arguments_object(&self) -> bool {
        self.state.parameters.actual_argument_count_local.is_some()
    }

    pub(in crate::backend::direct_wasm) fn emit_arguments_length(&mut self) {
        if let Some(actual_argument_count_local) = self.state.parameters.actual_argument_count_local
        {
            self.push_local_get(actual_argument_count_local);
        } else {
            self.push_i32_const(0);
        }
    }
}
