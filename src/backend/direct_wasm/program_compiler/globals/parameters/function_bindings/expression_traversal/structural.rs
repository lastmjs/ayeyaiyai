use super::super::super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn handle_array_parameter_expression(
        &self,
        elements: &[crate::ir::hir::ArrayElement],
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        for element in elements {
            match element {
                crate::ir::hir::ArrayElement::Expression(expression)
                | crate::ir::hir::ArrayElement::Spread(expression) => {
                    self.collect_parameter_bindings_from_expression(
                        expression,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                    );
                }
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn handle_object_parameter_expression(
        &self,
        entries: &[crate::ir::hir::ObjectEntry],
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        for entry in entries {
            match entry {
                crate::ir::hir::ObjectEntry::Data { key, value } => {
                    self.collect_parameter_bindings_from_expression(
                        key,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                    );
                    self.collect_parameter_bindings_from_expression(
                        value,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                    );
                }
                crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                    self.collect_parameter_bindings_from_expression(
                        key,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                    );
                    self.collect_parameter_bindings_from_expression(
                        getter,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                    );
                }
                crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                    self.collect_parameter_bindings_from_expression(
                        key,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                    );
                    self.collect_parameter_bindings_from_expression(
                        setter,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                    );
                }
                crate::ir::hir::ObjectEntry::Spread(expression) => {
                    self.collect_parameter_bindings_from_expression(
                        expression,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                    );
                }
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn handle_binary_parameter_expression(
        &self,
        left: &Expression,
        right: &Expression,
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        self.collect_parameter_bindings_from_expression(
            left,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        self.collect_parameter_bindings_from_expression(
            right,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
    }

    pub(in crate::backend::direct_wasm) fn handle_conditional_parameter_expression(
        &self,
        condition: &Expression,
        then_expression: &Expression,
        else_expression: &Expression,
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        self.collect_parameter_bindings_from_expression(
            condition,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        self.collect_parameter_bindings_from_expression(
            then_expression,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        self.collect_parameter_bindings_from_expression(
            else_expression,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
    }

    pub(in crate::backend::direct_wasm) fn handle_sequence_parameter_expression(
        &self,
        expressions: &[Expression],
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        for expression in expressions {
            self.collect_parameter_bindings_from_expression(
                expression,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
            );
        }
    }
}
