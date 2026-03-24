use super::{FunctionDeclaration, Statement};

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub strict: bool,
    pub functions: Vec<FunctionDeclaration>,
    pub statements: Vec<Statement>,
}
