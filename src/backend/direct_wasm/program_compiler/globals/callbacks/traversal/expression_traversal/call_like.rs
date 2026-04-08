use super::*;

impl DirectWasmCompiler {
    pub(super) fn collect_stateful_callback_bindings_from_call_like(
        &self,
        expression: &Expression,
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
        value_bindings: &HashMap<String, Expression>,
        object_state: &HashMap<String, ObjectValueBinding>,
        overwrite_existing: bool,
    ) -> bool {
        match expression {
            Expression::Call { callee, arguments } => {
                self.collect_stateful_callback_bindings_from_expression(
                    callee,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                self.register_callback_bindings_for_call_with_state(
                    callee,
                    arguments,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                for argument in arguments {
                    let argument = match argument {
                        CallArgument::Expression(argument) | CallArgument::Spread(argument) => {
                            argument
                        }
                    };
                    self.collect_stateful_callback_bindings_from_expression(
                        argument,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
                true
            }
            Expression::New { callee, arguments } | Expression::SuperCall { callee, arguments } => {
                self.collect_stateful_callback_bindings_from_expression(
                    callee,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                for argument in arguments {
                    let argument = match argument {
                        CallArgument::Expression(argument) | CallArgument::Spread(argument) => {
                            argument
                        }
                    };
                    self.collect_stateful_callback_bindings_from_expression(
                        argument,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
                true
            }
            _ => false,
        }
    }
}
