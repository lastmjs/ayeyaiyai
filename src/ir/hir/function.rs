use super::{Expression, Statement};

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
    AsyncGenerator,
}

impl FunctionKind {
    pub fn from_flags(is_generator: bool, is_async: bool) -> Self {
        match (is_generator, is_async) {
            (false, false) => Self::Ordinary,
            (true, false) => Self::Generator,
            (false, true) => Self::Async,
            (true, true) => Self::AsyncGenerator,
        }
    }

    pub fn is_generator(self) -> bool {
        matches!(self, Self::Generator | Self::AsyncGenerator)
    }

    pub fn is_async(self) -> bool {
        matches!(self, Self::Async | Self::AsyncGenerator)
    }
}

#[cfg(test)]
mod tests {
    use super::FunctionKind;

    #[test]
    fn function_kind_from_flags_preserves_async_generator_shape() {
        let kind = FunctionKind::from_flags(true, true);
        assert_eq!(kind, FunctionKind::AsyncGenerator);
        assert!(kind.is_async());
        assert!(kind.is_generator());
    }
}
