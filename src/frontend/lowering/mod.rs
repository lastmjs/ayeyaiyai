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
mod state;
mod statements;
mod support;
mod top_level;

pub(crate) use self::state::Lowerer;
use self::state::{
    AssignmentTarget, BindingScope, ForOfBinding, ForOfPatternBindingKind, LogicalAssignmentKind,
};
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
