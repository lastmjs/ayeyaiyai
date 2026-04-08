use super::*;

mod binary;
mod calls;
mod constructors;
mod contextual;
mod iterators;
mod literals;
mod member_assignments;
mod member_calls;
mod tracked_arrays;
mod unary;
mod updates;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_numeric_expression(
        &mut self,
        expression: &Expression,
    ) -> DirectResult<()> {
        match expression {
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::Bool(_)
            | Expression::Array(_)
            | Expression::Object(_) => self.emit_literal_expression(expression),
            Expression::Identifier(name) => self.emit_identifier_expression_value(name),
            Expression::Assign { name, value } => self.emit_assign_expression_value(name, value),
            Expression::Unary { op, expression } => self.emit_unary_expression(*op, expression),
            Expression::Member { object, property } => {
                self.emit_member_expression_value(object, property)
            }
            Expression::Sent => {
                self.push_i32_const(JS_UNDEFINED_TAG);
                Ok(())
            }
            Expression::NewTarget => {
                self.push_global_get(CURRENT_NEW_TARGET_GLOBAL_INDEX);
                Ok(())
            }
            Expression::SuperMember { property } => {
                self.emit_super_member_expression_value(property)
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => self.emit_assign_member_expression(object, property, value),
            Expression::AssignSuperMember { property, value } => {
                self.emit_assign_super_member_expression(property, value)
            }
            Expression::This => self.emit_this_expression_value(),
            Expression::EnumerateKeys(expression) => {
                self.emit_enumerate_keys_expression(expression)
            }
            Expression::GetIterator(expression) => self.emit_get_iterator_expression(expression),
            Expression::IteratorClose(expression) => {
                self.emit_iterator_close_expression(expression)
            }
            Expression::Await(expression) => self.emit_await_expression(expression),
            Expression::New { callee, arguments } => self.emit_new_expression(callee, arguments),
            Expression::Update { name, op, prefix } => {
                self.emit_update_expression(name, *op, *prefix)
            }
            Expression::Binary { op, left, right } => {
                self.emit_binary_expression_value(expression, *op, left, right)
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.emit_conditional_expression_value(condition, then_expression, else_expression)
            }
            Expression::Call { callee, arguments } => {
                self.emit_call_expression_dispatch(expression, callee, arguments)
            }
            Expression::Sequence(expressions) => self.emit_sequence_expression_value(expressions),
            Expression::SuperCall { callee, arguments } => {
                self.emit_super_call_expression(callee, arguments)
            }
        }
    }
}
