use super::*;

impl DirectWasmCompiler {
    pub(super) fn collect_stateful_callback_bindings_from_recursive_shapes(
        &self,
        expression: &Expression,
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
        value_bindings: &HashMap<String, Expression>,
        object_state: &HashMap<String, ObjectValueBinding>,
        overwrite_existing: bool,
    ) {
        match expression {
            Expression::Assign { value, .. }
            | Expression::Await(value)
            | Expression::EnumerateKeys(value)
            | Expression::GetIterator(value)
            | Expression::IteratorClose(value)
            | Expression::Unary {
                expression: value, ..
            } => self.collect_stateful_callback_bindings_from_expression(
                value,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
                value_bindings,
                object_state,
                overwrite_existing,
            ),
            Expression::Binary { left, right, .. } => {
                self.collect_stateful_callback_bindings_from_expression(
                    left,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                self.collect_stateful_callback_bindings_from_expression(
                    right,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.collect_stateful_callback_bindings_from_expression(
                    condition,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                self.collect_stateful_callback_bindings_from_expression(
                    then_expression,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                self.collect_stateful_callback_bindings_from_expression(
                    else_expression,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.collect_stateful_callback_bindings_from_expression(
                        expression,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
            }
            Expression::Update { .. }
            | Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::Identifier(_)
            | Expression::This
            | Expression::NewTarget
            | Expression::Sent => {}
            Expression::Call { .. }
            | Expression::New { .. }
            | Expression::SuperCall { .. }
            | Expression::Array(_)
            | Expression::Object(_)
            | Expression::Member { .. }
            | Expression::AssignMember { .. }
            | Expression::SuperMember { .. }
            | Expression::AssignSuperMember { .. } => {}
        }
    }
}
