use super::{Expression, SwitchCase};

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Declaration {
        body: Vec<Statement>,
    },
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

impl Statement {
    pub fn declared_binding_name(&self) -> Option<&str> {
        match self {
            Self::Var { name, .. } | Self::Let { name, .. } => Some(name),
            _ => None,
        }
    }
}
