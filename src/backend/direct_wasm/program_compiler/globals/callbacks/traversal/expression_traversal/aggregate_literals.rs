use super::*;

impl DirectWasmCompiler {
    pub(super) fn collect_stateful_callback_bindings_from_aggregate_literals(
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
            Expression::Array(elements) => {
                for element in elements {
                    let element = match element {
                        crate::ir::hir::ArrayElement::Expression(element)
                        | crate::ir::hir::ArrayElement::Spread(element) => element,
                    };
                    self.collect_stateful_callback_bindings_from_expression(
                        element,
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
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        crate::ir::hir::ObjectEntry::Data { key, value } => {
                            self.collect_stateful_callback_bindings_from_expression(
                                key,
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
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                            self.collect_stateful_callback_bindings_from_expression(
                                key,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                                value_bindings,
                                object_state,
                                overwrite_existing,
                            );
                            self.collect_stateful_callback_bindings_from_expression(
                                getter,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                                value_bindings,
                                object_state,
                                overwrite_existing,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                            self.collect_stateful_callback_bindings_from_expression(
                                key,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                                value_bindings,
                                object_state,
                                overwrite_existing,
                            );
                            self.collect_stateful_callback_bindings_from_expression(
                                setter,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                                value_bindings,
                                object_state,
                                overwrite_existing,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Spread(value) => {
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
                        }
                    }
                }
                true
            }
            _ => false,
        }
    }
}
