mod bindings;
mod blocks;
mod declarations;
mod expressions;
mod functions;
mod statements;

pub(crate) use self::bindings::{
    collect_module_declared_names, collect_pattern_binding_names, collect_var_decl_bound_names,
    ensure_module_lexical_names_are_unique,
};
pub(crate) use self::declarations::validate_declaration_syntax;
pub(crate) use self::expressions::validate_expression_syntax;
pub(crate) use self::functions::{validate_class_syntax, validate_function_syntax};
pub(crate) use self::statements::validate_statement_syntax;
