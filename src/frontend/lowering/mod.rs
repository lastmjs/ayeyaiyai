use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use anyhow::{Context, Result, bail, ensure};
use swc_common::Span;
use swc_ecma_ast::{
    ArrowExpr, AssignOp, AssignTarget, BinaryOp as SwcBinaryOp, BindingIdent, BlockStmt,
    BlockStmtOrExpr, BreakStmt, Callee, Class, ClassDecl, ClassMember, ClassMethod, Constructor,
    ContinueStmt, Decl, DefaultDecl, ExportDefaultDecl, Expr, ExprStmt, FnDecl, FnExpr, ForHead,
    ForInStmt, ForOfStmt, Function, LabeledStmt, Lit, MemberProp, MetaPropKind, MethodKind,
    ModuleDecl, ModuleItem, ObjectPatProp, ParamOrTsParamProp, Pat, Program as SwcProgram, Prop,
    PropName, PropOrSpread, SimpleAssignTarget, Stmt, SuperProp, SuperPropExpr, SwitchStmt,
    UnaryOp as SwcUnaryOp, UpdateOp as SwcUpdateOp, VarDeclKind, VarDeclOrExpr, WithStmt,
};

use crate::ir::hir::{
    ArrayElement, BinaryOp, CallArgument, Expression, FunctionDeclaration, FunctionKind,
    ObjectEntry, Parameter, Program, Statement, SwitchCase, UnaryOp, UpdateOp,
};

use super::{
    early_errors::{
        collect_pattern_binding_names, function_has_use_strict_directive,
        script_has_use_strict_directive,
    },
    modules::resolution::resolve_module_specifier,
};

mod classes;
mod core;
mod declarations;
mod expressions;
mod functions;
mod generators;
mod loops;
mod patterns;
mod statements;
mod support;
mod top_level;

use self::support::{
    assert_throws_call, collect_for_of_binding_names, collect_for_per_iteration_bindings,
    collect_function_scope_binding_names, collect_parameter_binding_names, collect_switch_bindings,
    console_log_arguments, expected_argument_count, function_has_simple_parameter_list,
    lower_binary_operator, lower_constructor_parameters, lower_function_kind, lower_parameter,
    lower_parameter_patterns, lower_parameters, lower_unary_operator, lower_update_operator,
    parse_bigint_literal, pattern_name_hint, static_member_property_name, template_quasi_text,
};
pub(crate) use self::support::{
    asyncify_statements, collect_direct_statement_lexical_bindings, data_property_descriptor,
    define_property_statement, getter_property_descriptor, setter_property_descriptor,
};

#[derive(Default)]
pub(super) struct Lowerer {
    pub(super) source_text: Option<String>,
    pub(super) functions: Vec<FunctionDeclaration>,
    pub(super) next_function_expression_id: usize,
    pub(super) next_temporary_id: usize,
    binding_scopes: Vec<BindingScope>,
    pub(super) active_binding_counts: HashMap<String, usize>,
    pub(super) private_name_scopes: Vec<HashMap<String, String>>,
    pub(super) constructor_super_stack: Vec<Option<String>>,
    pub(super) strict_modes: Vec<bool>,
    pub(super) module_mode: bool,
    pub(super) current_module_path: Option<PathBuf>,
    pub(super) module_index_lookup: HashMap<PathBuf, usize>,
}

#[derive(Default)]
struct BindingScope {
    names: Vec<String>,
    renames: HashMap<String, String>,
}

enum AssignmentTarget {
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
    fn as_expression(&self) -> Expression {
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

    fn into_statement(self, value: Expression) -> Statement {
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

    fn into_expression(self, value: Expression) -> Expression {
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

struct ForOfBinding {
    before_loop: Vec<Statement>,
    per_iteration: Vec<Statement>,
}

#[derive(Clone, Copy)]
enum ForOfPatternBindingKind {
    Assignment,
    Var,
    Lexical { mutable: bool },
}

#[derive(Clone, Copy)]
enum LogicalAssignmentKind {
    And,
    Or,
    Nullish,
}
