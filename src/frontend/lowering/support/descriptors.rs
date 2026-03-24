use super::super::*;

pub(crate) fn define_property_statement(
    target: Expression,
    property: Expression,
    descriptor: Expression,
) -> Statement {
    Statement::Expression(Expression::Call {
        callee: Box::new(Expression::Member {
            object: Box::new(Expression::Identifier("Object".to_string())),
            property: Box::new(Expression::String("defineProperty".to_string())),
        }),
        arguments: vec![
            CallArgument::Expression(target),
            CallArgument::Expression(property),
            CallArgument::Expression(descriptor),
        ],
    })
}

pub(crate) fn data_property_descriptor(
    value: Expression,
    writable: bool,
    enumerable: bool,
    configurable: bool,
) -> Expression {
    Expression::Object(vec![
        ObjectEntry::Data {
            key: Expression::String("value".to_string()),
            value,
        },
        ObjectEntry::Data {
            key: Expression::String("writable".to_string()),
            value: Expression::Bool(writable),
        },
        ObjectEntry::Data {
            key: Expression::String("enumerable".to_string()),
            value: Expression::Bool(enumerable),
        },
        ObjectEntry::Data {
            key: Expression::String("configurable".to_string()),
            value: Expression::Bool(configurable),
        },
    ])
}

pub(crate) fn getter_property_descriptor(
    getter: Expression,
    enumerable: bool,
    configurable: bool,
) -> Expression {
    Expression::Object(vec![
        ObjectEntry::Data {
            key: Expression::String("get".to_string()),
            value: getter,
        },
        ObjectEntry::Data {
            key: Expression::String("enumerable".to_string()),
            value: Expression::Bool(enumerable),
        },
        ObjectEntry::Data {
            key: Expression::String("configurable".to_string()),
            value: Expression::Bool(configurable),
        },
    ])
}

pub(crate) fn setter_property_descriptor(
    setter: Expression,
    enumerable: bool,
    configurable: bool,
) -> Expression {
    Expression::Object(vec![
        ObjectEntry::Data {
            key: Expression::String("set".to_string()),
            value: setter,
        },
        ObjectEntry::Data {
            key: Expression::String("enumerable".to_string()),
            value: Expression::Bool(enumerable),
        },
        ObjectEntry::Data {
            key: Expression::String("configurable".to_string()),
            value: Expression::Bool(configurable),
        },
    ])
}
