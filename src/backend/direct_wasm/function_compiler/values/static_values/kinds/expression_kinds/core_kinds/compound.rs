use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn infer_compound_expression_kind(
        &self,
        expression: &Expression,
    ) -> Option<StaticValueKind> {
        match expression {
            Expression::Assign { value, .. } => self.infer_value_kind(value),
            Expression::AssignMember { value, .. }
            | Expression::AssignSuperMember { value, .. } => self.infer_value_kind(value),
            Expression::Sequence(expressions) => expressions.last().and_then(|last| {
                self.infer_value_kind(last)
                    .or(Some(StaticValueKind::Unknown))
            }),
            Expression::Call { callee, arguments } => {
                self.infer_call_expression_kind(expression, callee, arguments)
            }
            Expression::New { .. } => Some(StaticValueKind::Object),
            Expression::NewTarget => Some(StaticValueKind::Unknown),
            Expression::Member { object, property } => {
                self.infer_member_expression_kind(object, property)
            }
            Expression::SuperMember { .. } => Some(StaticValueKind::Unknown),
            Expression::Update { .. } => Some(StaticValueKind::Number),
            Expression::Array(_) | Expression::Object(_) => Some(StaticValueKind::Object),
            Expression::This => Some(StaticValueKind::Object),
            Expression::Sent
            | Expression::Await(_)
            | Expression::IteratorClose(_)
            | Expression::SuperCall { .. } => Some(StaticValueKind::Undefined),
            Expression::EnumerateKeys(_) | Expression::GetIterator(_) => {
                Some(StaticValueKind::Object)
            }
            _ => None,
        }
    }
}
