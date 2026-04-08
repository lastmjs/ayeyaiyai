use super::*;

pub(in crate::backend::direct_wasm) fn collect_inline_function_summary(
    function: &FunctionDeclaration,
) -> Option<InlineFunctionSummary> {
    let mut summary = InlineFunctionSummary::default();
    let parameter_names = function
        .params
        .iter()
        .map(|param| param.name.clone())
        .collect::<HashSet<_>>();
    let mut local_bindings = HashMap::new();
    for statement in &function.body {
        match statement {
            Statement::Var { name, value } | Statement::Let { name, value, .. } => {
                if parameter_names.contains(name) {
                    return None;
                }
                local_bindings.insert(
                    name.clone(),
                    substitute_inline_summary_bindings(value, &local_bindings),
                );
            }
            Statement::Assign { name, value } => {
                if parameter_names.contains(name) {
                    return None;
                }
                if local_bindings.contains_key(name) {
                    return None;
                }
                summary.effects.push(InlineFunctionEffect::Assign {
                    name: name.clone(),
                    value: substitute_inline_summary_bindings(value, &local_bindings),
                });
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                let object = substitute_inline_summary_bindings(object, &local_bindings);
                let property = substitute_inline_summary_bindings(property, &local_bindings);
                let value = substitute_inline_summary_bindings(value, &local_bindings);
                if !function.mapped_arguments
                    && matches!(&object, Expression::Identifier(name) if name == "arguments")
                    && inline_summary_side_effect_free_expression(&property)
                    && inline_summary_side_effect_free_expression(&value)
                {
                    continue;
                }
                summary
                    .effects
                    .push(InlineFunctionEffect::Expression(Expression::AssignMember {
                        object: Box::new(object),
                        property: Box::new(property),
                        value: Box::new(value),
                    }));
            }
            Statement::Expression(Expression::Update { name, op, prefix }) => {
                if function.params.iter().any(|param| param.name == *name)
                    || local_bindings.contains_key(name)
                {
                    return None;
                }
                summary.effects.push(InlineFunctionEffect::Update {
                    name: name.clone(),
                    op: *op,
                    prefix: *prefix,
                });
            }
            Statement::Expression(expression) => {
                summary.effects.push(InlineFunctionEffect::Expression(
                    substitute_inline_summary_bindings(expression, &local_bindings),
                ))
            }
            Statement::Return(value) => {
                if summary.return_value.is_some() {
                    return None;
                }
                summary.return_value =
                    Some(substitute_inline_summary_bindings(value, &local_bindings));
            }
            Statement::Block { body } if body.is_empty() => {}
            _ => return None,
        }
    }

    Some(summary)
}
