use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn find_iterator_source_expression_in_statements(
        statements: &[Statement],
        iterator_name: &str,
    ) -> Option<Expression> {
        for statement in statements {
            match statement {
                Statement::Declaration { body }
                | Statement::Block { body }
                | Statement::Labeled { body, .. }
                | Statement::With { body, .. } => {
                    if let Some(iterated) =
                        Self::find_iterator_source_expression_in_statements(body, iterator_name)
                    {
                        return Some(iterated);
                    }
                }
                Statement::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    if let Some(iterated) = Self::find_iterator_source_expression_in_statements(
                        then_branch,
                        iterator_name,
                    ) {
                        return Some(iterated);
                    }
                    if let Some(iterated) = Self::find_iterator_source_expression_in_statements(
                        else_branch,
                        iterator_name,
                    ) {
                        return Some(iterated);
                    }
                }
                Statement::Try {
                    body,
                    catch_setup,
                    catch_body,
                    ..
                } => {
                    if let Some(iterated) =
                        Self::find_iterator_source_expression_in_statements(body, iterator_name)
                    {
                        return Some(iterated);
                    }
                    if let Some(iterated) = Self::find_iterator_source_expression_in_statements(
                        catch_setup,
                        iterator_name,
                    ) {
                        return Some(iterated);
                    }
                    if let Some(iterated) = Self::find_iterator_source_expression_in_statements(
                        catch_body,
                        iterator_name,
                    ) {
                        return Some(iterated);
                    }
                }
                Statement::Switch { cases, .. } => {
                    for case in cases {
                        if let Some(iterated) = Self::find_iterator_source_expression_in_statements(
                            &case.body,
                            iterator_name,
                        ) {
                            return Some(iterated);
                        }
                    }
                }
                Statement::For { init, body, .. } => {
                    if let Some(iterated) =
                        Self::find_iterator_source_expression_in_statements(init, iterator_name)
                    {
                        return Some(iterated);
                    }
                    if let Some(iterated) =
                        Self::find_iterator_source_expression_in_statements(body, iterator_name)
                    {
                        return Some(iterated);
                    }
                }
                Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
                    if let Some(iterated) =
                        Self::find_iterator_source_expression_in_statements(body, iterator_name)
                    {
                        return Some(iterated);
                    }
                }
                Statement::Var { name, value }
                | Statement::Let { name, value, .. }
                | Statement::Assign { name, value }
                    if name == iterator_name =>
                {
                    if let Expression::GetIterator(iterated) = value {
                        return Some((**iterated).clone());
                    }
                }
                _ => {}
            }
        }
        None
    }
}
