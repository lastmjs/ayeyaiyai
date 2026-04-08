use super::*;

impl DirectWasmCompiler {
    pub(super) fn collect_stateful_callback_bindings_from_member_access(
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
            Expression::Member { object, property }
            | Expression::AssignMember {
                object, property, ..
            } => {
                self.collect_stateful_callback_bindings_from_expression(
                    object,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                self.collect_stateful_callback_bindings_from_expression(
                    property,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                true
            }
            Expression::SuperMember { property } => {
                self.collect_stateful_callback_bindings_from_expression(
                    property,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                true
            }
            Expression::AssignSuperMember { property, value } => {
                self.collect_stateful_callback_bindings_from_expression(
                    property,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                self.collect_stateful_callback_bindings_from_expression(
                    value,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                true
            }
            _ => false,
        }
    }
}
