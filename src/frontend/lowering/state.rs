use super::*;

#[derive(Default)]
pub(crate) struct Lowerer {
    pub(crate) source_text: Option<String>,
    pub(crate) functions: Vec<FunctionDeclaration>,
    pub(super) next_function_expression_id: usize,
    pub(super) next_temporary_id: usize,
    pub(super) binding_scopes: Vec<BindingScope>,
    pub(super) active_binding_counts: HashMap<String, usize>,
    pub(super) private_name_scopes: Vec<HashMap<String, String>>,
    pub(super) constructor_super_stack: Vec<Option<String>>,
    pub(crate) strict_modes: Vec<bool>,
    pub(crate) module_mode: bool,
    pub(crate) current_module_path: Option<PathBuf>,
    pub(crate) module_index_lookup: HashMap<PathBuf, usize>,
}

#[derive(Default)]
pub(super) struct BindingScope {
    pub(super) names: Vec<String>,
    pub(super) renames: HashMap<String, String>,
}

pub(super) enum AssignmentTarget {
    Identifier(String),
    Member {
        object: Expression,
        property: Expression,
    },
    SuperMember {
        property: Expression,
    },
}

impl AssignmentTarget {
    pub(super) fn as_expression(&self) -> Expression {
        match self {
            AssignmentTarget::Identifier(name) => Expression::Identifier(name.clone()),
            AssignmentTarget::Member { object, property } => Expression::Member {
                object: Box::new(object.clone()),
                property: Box::new(property.clone()),
            },
            AssignmentTarget::SuperMember { property } => Expression::SuperMember {
                property: Box::new(property.clone()),
            },
        }
    }

    pub(super) fn into_statement(self, value: Expression) -> Statement {
        match self {
            AssignmentTarget::Identifier(name) => Statement::Assign { name, value },
            AssignmentTarget::Member { object, property } => Statement::AssignMember {
                object,
                property,
                value,
            },
            AssignmentTarget::SuperMember { property } => {
                Statement::Expression(Expression::AssignSuperMember {
                    property: Box::new(property),
                    value: Box::new(value),
                })
            }
        }
    }

    pub(super) fn into_expression(self, value: Expression) -> Expression {
        match self {
            AssignmentTarget::Identifier(name) => Expression::Assign {
                name,
                value: Box::new(value),
            },
            AssignmentTarget::Member { object, property } => Expression::AssignMember {
                object: Box::new(object),
                property: Box::new(property),
                value: Box::new(value),
            },
            AssignmentTarget::SuperMember { property } => Expression::AssignSuperMember {
                property: Box::new(property),
                value: Box::new(value),
            },
        }
    }
}

pub(super) struct ForOfBinding {
    pub(super) before_loop: Vec<Statement>,
    pub(super) per_iteration: Vec<Statement>,
}

#[derive(Clone, Copy)]
pub(super) enum ForOfPatternBindingKind {
    Assignment,
    Var,
    Lexical { mutable: bool },
}

#[derive(Clone, Copy)]
pub(super) enum LogicalAssignmentKind {
    And,
    Or,
    Nullish,
}
