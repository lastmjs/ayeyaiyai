use super::{ArrayElement, BinaryOp, CallArgument, ObjectEntry, UnaryOp, UpdateOp};

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Number(f64),
    BigInt(String),
    String(String),
    Bool(bool),
    Null,
    Undefined,
    NewTarget,
    Array(Vec<ArrayElement>),
    Object(Vec<ObjectEntry>),
    Identifier(String),
    This,
    Sent,
    Member {
        object: Box<Expression>,
        property: Box<Expression>,
    },
    SuperMember {
        property: Box<Expression>,
    },
    Assign {
        name: String,
        value: Box<Expression>,
    },
    AssignMember {
        object: Box<Expression>,
        property: Box<Expression>,
        value: Box<Expression>,
    },
    AssignSuperMember {
        property: Box<Expression>,
        value: Box<Expression>,
    },
    Await(Box<Expression>),
    EnumerateKeys(Box<Expression>),
    GetIterator(Box<Expression>),
    IteratorClose(Box<Expression>),
    Unary {
        op: UnaryOp,
        expression: Box<Expression>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Conditional {
        condition: Box<Expression>,
        then_expression: Box<Expression>,
        else_expression: Box<Expression>,
    },
    Sequence(Vec<Expression>),
    Call {
        callee: Box<Expression>,
        arguments: Vec<CallArgument>,
    },
    SuperCall {
        callee: Box<Expression>,
        arguments: Vec<CallArgument>,
    },
    New {
        callee: Box<Expression>,
        arguments: Vec<CallArgument>,
    },
    Update {
        name: String,
        op: UpdateOp,
        prefix: bool,
    },
}
