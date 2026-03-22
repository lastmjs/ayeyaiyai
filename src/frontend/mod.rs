use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, ensure, Context, Result};
use swc_common::{source_map::SmallPos, sync::Lrc, FileName, SourceMap, Span};
use swc_ecma_ast::{
    ArrowExpr, AssignOp, AssignTarget, BinaryOp as SwcBinaryOp, BindingIdent, BlockStmt,
    BlockStmtOrExpr, BreakStmt, Callee, Class, ClassDecl, ClassMember, ClassMethod, Constructor,
    ContinueStmt, Decl, DefaultDecl, ExportDefaultDecl, ExportSpecifier, Expr, ExprStmt, FnDecl,
    FnExpr, ForHead, ForInStmt, ForOfStmt, Function, ImportDecl, ImportSpecifier, LabeledStmt,
    Lit, MemberProp, MetaPropKind, MethodKind, Module, ModuleDecl, ModuleExportName, ModuleItem,
    ObjectLit, ObjectPatProp, ParamOrTsParamProp, Pat, Program as SwcProgram, Prop, PropName,
    PropOrSpread, SimpleAssignTarget, Stmt, SuperProp, SuperPropExpr, SwitchStmt,
    UnaryOp as SwcUnaryOp, UpdateOp as SwcUpdateOp, VarDeclKind, VarDeclOrExpr, WithStmt,
};
use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax};

use crate::ir::hir::{
    ArrayElement, BinaryOp, CallArgument, Expression, FunctionDeclaration, FunctionKind,
    ObjectEntry, Parameter, Program, Statement, SwitchCase, UnaryOp, UpdateOp,
};

mod strict_mode;
mod syntax;

use strict_mode::{
    function_has_use_strict_directive, script_has_use_strict_directive,
    validate_strict_mode_early_errors_in_module_items,
    validate_strict_mode_early_errors_in_statements,
};
use syntax::{
    collect_module_declared_names, collect_pattern_binding_names, collect_var_decl_bound_names,
    ensure_module_lexical_names_are_unique, validate_class_syntax, validate_declaration_syntax,
    validate_expression_syntax, validate_function_syntax, validate_statement_syntax,
};

include!("parse.rs");
include!("bundle.rs");
include!("lowering.rs");

#[cfg(test)]
mod tests;
