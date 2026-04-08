use super::*;

thread_local! {
    static ACTIVE_MATERIALIZATION_SHAPES: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

struct StructuralMaterializationGuard {
    key: String,
}

impl Drop for StructuralMaterializationGuard {
    fn drop(&mut self) {
        ACTIVE_MATERIALIZATION_SHAPES.with(|active| {
            active.borrow_mut().remove(&self.key);
        });
    }
}

#[path = "materialization/compound.rs"]
mod compound;
#[path = "materialization/identifiers.rs"]
mod identifiers;
#[path = "materialization/members.rs"]
mod members;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn materialize_static_expression(
        &self,
        expression: &Expression,
    ) -> Expression {
        let structural_key = format!("{expression:?}");
        let inserted = ACTIVE_MATERIALIZATION_SHAPES
            .with(|active| active.borrow_mut().insert(structural_key.clone()));
        if !inserted {
            return expression.clone();
        }
        let _structural_guard = StructuralMaterializationGuard {
            key: structural_key,
        };
        let guard_key = expression as *const Expression as usize;
        {
            let mut active = self
                .state
                .speculation
                .static_semantics
                .materializing_expression_keys
                .borrow_mut();
            if !active.insert(guard_key) {
                return expression.clone();
            }
        }
        let _guard = MaterializationGuard {
            active: &self
                .state
                .speculation
                .static_semantics
                .materializing_expression_keys,
            key: guard_key,
        };
        match expression {
            Expression::Identifier(name) => {
                self.materialize_identifier_expression(name, expression)
            }
            Expression::Member { object, property } => {
                self.materialize_member_expression(object, property)
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.materialize_conditional_expression(condition, then_expression, else_expression)
            }
            Expression::Call { callee, arguments } => {
                self.materialize_call_expression(expression, callee, arguments)
            }
            _ => self.materialize_recursive_expression_default(expression),
        }
    }
}
