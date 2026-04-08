use super::*;

#[path = "expressions/aggregates.rs"]
mod aggregates;
#[path = "expressions/calls.rs"]
mod calls;
#[path = "expressions/core.rs"]
mod core;
#[path = "expressions/mutations.rs"]
mod mutations;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn evaluate_bound_snapshot_expression(
        &self,
        expression: &Expression,
        bindings: &mut HashMap<String, Expression>,
        current_function_name: Option<&str>,
    ) -> Option<Expression> {
        let _guard = BoundSnapshotExpressionGuard::enter(expression, current_function_name)?;
        match expression {
            Expression::Identifier(name) => {
                self.evaluate_bound_snapshot_identifier(name, expression, bindings)
            }
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined => Some(expression.clone()),
            Expression::This => self.evaluate_bound_snapshot_this_expression(
                expression,
                bindings,
                current_function_name,
            ),
            Expression::Binary { op, left, right } => self
                .evaluate_bound_snapshot_binary_expression(
                    *op,
                    left,
                    right,
                    bindings,
                    current_function_name,
                ),
            Expression::Member { object, property } => self
                .evaluate_bound_snapshot_member_expression(
                    object,
                    property,
                    bindings,
                    current_function_name,
                ),
            Expression::Assign { name, value } => self.evaluate_bound_snapshot_assign_expression(
                name,
                value,
                bindings,
                current_function_name,
            ),
            Expression::AssignMember {
                object,
                property,
                value,
            } => self.evaluate_bound_snapshot_assign_member_expression(
                object,
                property,
                value,
                bindings,
                current_function_name,
            ),
            Expression::AssignSuperMember { property, value } => self
                .evaluate_bound_snapshot_assign_super_member_expression(
                    property,
                    value,
                    bindings,
                    current_function_name,
                ),
            Expression::Call { callee, arguments }
            | Expression::SuperCall { callee, arguments }
            | Expression::New { callee, arguments } => self
                .evaluate_bound_snapshot_call_expression(
                    callee,
                    arguments,
                    bindings,
                    current_function_name,
                ),
            Expression::Array(elements) => self.evaluate_bound_snapshot_array_literal(
                elements,
                bindings,
                current_function_name,
            ),
            Expression::Object(entries) => self.evaluate_bound_snapshot_object_literal(
                entries,
                bindings,
                current_function_name,
            ),
            Expression::Update { name, op, prefix } => {
                self.evaluate_bound_snapshot_update_expression(name, *op, *prefix, bindings)
            }
            _ => None,
        }
    }
}
