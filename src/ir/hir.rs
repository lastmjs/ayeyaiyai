#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub strict: bool,
    pub functions: Vec<FunctionDeclaration>,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDeclaration {
    pub name: String,
    pub top_level_binding: Option<String>,
    pub params: Vec<Parameter>,
    pub body: Vec<Statement>,
    pub register_global: bool,
    pub kind: FunctionKind,
    pub self_binding: Option<String>,
    pub mapped_arguments: bool,
    pub strict: bool,
    pub lexical_this: bool,
    pub length: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub default: Option<Expression>,
    pub rest: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionKind {
    Ordinary,
    Generator,
    Async,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectEntry {
    Data { key: Expression, value: Expression },
    Getter { key: Expression, getter: Expression },
    Setter { key: Expression, setter: Expression },
    Spread(Expression),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayElement {
    Expression(Expression),
    Spread(Expression),
}

#[derive(Debug, Clone, PartialEq)]
pub enum CallArgument {
    Expression(Expression),
    Spread(Expression),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SwitchCase {
    pub test: Option<Expression>,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Block {
        body: Vec<Statement>,
    },
    Labeled {
        labels: Vec<String>,
        body: Vec<Statement>,
    },
    Var {
        name: String,
        value: Expression,
    },
    Let {
        name: String,
        mutable: bool,
        value: Expression,
    },
    Assign {
        name: String,
        value: Expression,
    },
    AssignMember {
        object: Expression,
        property: Expression,
        value: Expression,
    },
    Print {
        values: Vec<Expression>,
    },
    Expression(Expression),
    Throw(Expression),
    Return(Expression),
    Break {
        label: Option<String>,
    },
    Continue {
        label: Option<String>,
    },
    Yield {
        value: Expression,
    },
    YieldDelegate {
        value: Expression,
    },
    With {
        object: Expression,
        body: Vec<Statement>,
    },
    If {
        condition: Expression,
        then_branch: Vec<Statement>,
        else_branch: Vec<Statement>,
    },
    Try {
        body: Vec<Statement>,
        catch_binding: Option<String>,
        catch_setup: Vec<Statement>,
        catch_body: Vec<Statement>,
    },
    Switch {
        labels: Vec<String>,
        bindings: Vec<String>,
        discriminant: Expression,
        cases: Vec<SwitchCase>,
    },
    For {
        labels: Vec<String>,
        init: Vec<Statement>,
        per_iteration_bindings: Vec<String>,
        condition: Option<Expression>,
        update: Option<Expression>,
        break_hook: Option<Expression>,
        body: Vec<Statement>,
    },
    While {
        labels: Vec<String>,
        condition: Expression,
        break_hook: Option<Expression>,
        body: Vec<Statement>,
    },
    DoWhile {
        labels: Vec<String>,
        condition: Expression,
        break_hook: Option<Expression>,
        body: Vec<Statement>,
    },
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Negate,
    Plus,
    Not,
    BitwiseNot,
    TypeOf,
    Void,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateOp {
    Increment,
    Decrement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Exponentiate,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    LeftShift,
    RightShift,
    UnsignedRightShift,
    In,
    InstanceOf,
    LooseEqual,
    LooseNotEqual,
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LogicalAnd,
    LogicalOr,
    NullishCoalescing,
}
