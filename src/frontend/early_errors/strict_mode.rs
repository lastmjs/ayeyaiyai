mod bindings;
mod directives;
mod expressions;
mod functions;
mod statements;

pub(crate) use self::directives::{
    function_has_use_strict_directive, script_has_use_strict_directive,
};
pub(crate) use self::statements::{
    validate_strict_mode_early_errors_in_module_items,
    validate_strict_mode_early_errors_in_statements,
};
