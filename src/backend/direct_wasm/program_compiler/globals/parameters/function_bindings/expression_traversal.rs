use super::*;

#[path = "expression_traversal/assignments.rs"]
mod assignments;
#[path = "expression_traversal/calls.rs"]
mod calls;
#[path = "expression_traversal/structural.rs"]
mod structural;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn collect_parameter_bindings_from_expression(
        &self,
        expression: &Expression,
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        match expression {
            Expression::Call { callee, arguments } => self.handle_call_parameter_expression(
                callee,
                arguments,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
            ),
            Expression::Assign { name, value } => self.handle_assign_parameter_expression(
                name,
                value,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
            ),
            Expression::AssignMember {
                object,
                property,
                value,
            } => self.handle_assign_member_parameter_expression(
                object,
                property,
                value,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
            ),
            Expression::AssignSuperMember { property, value } => {
                self.collect_parameter_bindings_from_expression(
                    property,
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
            Expression::Member { object, property } => self.handle_member_parameter_expression(
                object,
                property,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
            ),
            Expression::SuperMember { property } => self
                .collect_parameter_bindings_from_expression(
                    property,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                ),
            Expression::Unary { expression, .. }
            | Expression::EnumerateKeys(expression)
            | Expression::GetIterator(expression)
            | Expression::IteratorClose(expression)
            | Expression::Await(expression) => self.collect_parameter_bindings_from_expression(
                expression,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
            ),
            Expression::Array(elements) => self.handle_array_parameter_expression(
                elements,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
            ),
            Expression::Object(entries) => self.handle_object_parameter_expression(
                entries,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
            ),
            Expression::Binary { left, right, .. } => self.handle_binary_parameter_expression(
                left,
                right,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
            ),
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => self.handle_conditional_parameter_expression(
                condition,
                then_expression,
                else_expression,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
            ),
            Expression::Sequence(expressions) => self.handle_sequence_parameter_expression(
                expressions,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
            ),
            Expression::New { callee, arguments } | Expression::SuperCall { callee, arguments } => {
                self.handle_construct_parameter_expression(
                    callee,
                    arguments,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                )
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
            | Expression::Sent
            | Expression::NewTarget => {}
        }
    }
}
