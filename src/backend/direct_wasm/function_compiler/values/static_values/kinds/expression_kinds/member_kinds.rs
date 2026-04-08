use super::*;

#[path = "member_kinds/builtin_members.rs"]
mod builtin_members;
#[path = "member_kinds/object_members.rs"]
mod object_members;
#[path = "member_kinds/special_members.rs"]
mod special_members;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn infer_member_expression_kind(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<StaticValueKind> {
        self.infer_special_member_kind(object, property)
            .or_else(|| self.infer_builtin_member_kind(object, property))
            .or_else(|| self.infer_object_member_kind(object, property))
            .or(Some(StaticValueKind::Unknown))
    }
}
