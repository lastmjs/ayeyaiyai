use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn collect_direct_parameter_get_iterator_name(
        expression: &Expression,
        param_names: &HashSet<String>,
        consumed_names: &mut HashSet<String>,
    ) {
        if let Expression::GetIterator(value) = expression
            && let Expression::Identifier(name) = value.as_ref()
            && param_names.contains(name)
        {
            consumed_names.insert(name.clone());
        }
    }
}
