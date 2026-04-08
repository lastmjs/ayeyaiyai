use super::*;

pub(in crate::backend::direct_wasm) fn materialize_array_elements(
    elements: &[ArrayElement],
    allow_spread: bool,
    recurse: impl Fn(&Expression) -> Option<Expression>,
) -> Option<Vec<ArrayElement>> {
    elements
        .iter()
        .map(|element| match element {
            ArrayElement::Expression(expression) => {
                Some(ArrayElement::Expression(recurse(expression)?))
            }
            ArrayElement::Spread(expression) if allow_spread => {
                Some(ArrayElement::Spread(recurse(expression)?))
            }
            ArrayElement::Spread(_) => None,
        })
        .collect::<Option<Vec<_>>>()
}

pub(in crate::backend::direct_wasm) fn materialize_structural_expression<Environment>(
    expression: &Expression,
    allow_accessors: bool,
    allow_spread: bool,
    environment: &Environment,
    recurse: &dyn Fn(&Expression, &Environment) -> Option<Expression>,
) -> Option<Expression> {
    match expression {
        Expression::Array(elements) => Some(Expression::Array(materialize_array_elements(
            elements,
            allow_spread,
            |expression| recurse(expression, environment),
        )?)),
        Expression::Object(entries) => Some(Expression::Object(materialize_object_entries(
            entries,
            allow_accessors,
            allow_spread,
            |expression| recurse(expression, environment),
        )?)),
        _ => None,
    }
}

pub(in crate::backend::direct_wasm) fn materialize_call_arguments(
    arguments: &[CallArgument],
    recurse: &dyn Fn(&Expression) -> Option<Expression>,
) -> Option<Vec<CallArgument>> {
    arguments
        .iter()
        .map(|argument| match argument {
            CallArgument::Expression(expression) => {
                Some(CallArgument::Expression(recurse(expression)?))
            }
            CallArgument::Spread(expression) => Some(CallArgument::Spread(recurse(expression)?)),
        })
        .collect::<Option<Vec<_>>>()
}

pub(in crate::backend::direct_wasm) fn materialize_recursive_expression(
    expression: &Expression,
    allow_accessors: bool,
    allow_spread: bool,
    recurse: &dyn Fn(&Expression) -> Option<Expression>,
) -> Option<Expression> {
    match expression {
        Expression::Unary { op, expression } => Some(Expression::Unary {
            op: *op,
            expression: Box::new(recurse(expression)?),
        }),
        Expression::Binary { op, left, right } => Some(Expression::Binary {
            op: *op,
            left: Box::new(recurse(left)?),
            right: Box::new(recurse(right)?),
        }),
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => Some(Expression::Conditional {
            condition: Box::new(recurse(condition)?),
            then_expression: Box::new(recurse(then_expression)?),
            else_expression: Box::new(recurse(else_expression)?),
        }),
        Expression::Sequence(expressions) => Some(Expression::Sequence(
            expressions
                .iter()
                .map(recurse)
                .collect::<Option<Vec<_>>>()?,
        )),
        Expression::Array(_) | Expression::Object(_) => materialize_structural_expression(
            expression,
            allow_accessors,
            allow_spread,
            &(),
            &|expression, _| recurse(expression),
        ),
        Expression::Assign { name, value } => Some(Expression::Assign {
            name: name.clone(),
            value: Box::new(recurse(value)?),
        }),
        Expression::AssignMember {
            object,
            property,
            value,
        } => Some(Expression::AssignMember {
            object: Box::new(recurse(object)?),
            property: Box::new(recurse(property)?),
            value: Box::new(recurse(value)?),
        }),
        Expression::AssignSuperMember { property, value } => Some(Expression::AssignSuperMember {
            property: Box::new(recurse(property)?),
            value: Box::new(recurse(value)?),
        }),
        Expression::Await(value) => Some(Expression::Await(Box::new(recurse(value)?))),
        Expression::EnumerateKeys(value) => {
            Some(Expression::EnumerateKeys(Box::new(recurse(value)?)))
        }
        Expression::GetIterator(value) => Some(Expression::GetIterator(Box::new(recurse(value)?))),
        Expression::IteratorClose(value) => {
            Some(Expression::IteratorClose(Box::new(recurse(value)?)))
        }
        Expression::Call { callee, arguments } => Some(Expression::Call {
            callee: Box::new(recurse(callee)?),
            arguments: materialize_call_arguments(arguments, recurse)?,
        }),
        Expression::New { callee, arguments } => Some(Expression::New {
            callee: Box::new(recurse(callee)?),
            arguments: materialize_call_arguments(arguments, recurse)?,
        }),
        _ => None,
    }
}

pub(in crate::backend::direct_wasm) fn materialize_object_entries(
    entries: &[ObjectEntry],
    allow_accessors: bool,
    allow_spread: bool,
    recurse: impl Fn(&Expression) -> Option<Expression>,
) -> Option<Vec<ObjectEntry>> {
    entries
        .iter()
        .map(|entry| match entry {
            ObjectEntry::Data { key, value } => Some(ObjectEntry::Data {
                key: recurse(key)?,
                value: recurse(value)?,
            }),
            ObjectEntry::Getter { key, getter } if allow_accessors => Some(ObjectEntry::Getter {
                key: recurse(key)?,
                getter: recurse(getter)?,
            }),
            ObjectEntry::Setter { key, setter } if allow_accessors => Some(ObjectEntry::Setter {
                key: recurse(key)?,
                setter: recurse(setter)?,
            }),
            ObjectEntry::Spread(expression) if allow_spread => {
                Some(ObjectEntry::Spread(recurse(expression)?))
            }
            ObjectEntry::Getter { .. } | ObjectEntry::Setter { .. } | ObjectEntry::Spread(_) => {
                None
            }
        })
        .collect::<Option<Vec<_>>>()
}
