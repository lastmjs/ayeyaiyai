use super::super::*;
use super::{
    bindings::{
        validate_property_name_strict_mode_early_errors,
        validate_strict_mode_early_errors_in_pattern,
        validate_strict_mode_early_errors_in_variable_declaration,
    },
    directives::function_has_use_strict_directive,
    expressions::validate_strict_mode_early_errors_in_expression,
    statements::validate_strict_mode_early_errors_in_statements,
};

pub(super) fn validate_strict_mode_early_errors_in_declaration(
    declaration: &Decl,
    strict: bool,
) -> Result<()> {
    match declaration {
        Decl::Fn(function) => {
            validate_strict_mode_early_errors_in_function(&function.function, strict)?
        }
        Decl::Class(class) => validate_strict_mode_early_errors_in_class(&class.class, strict)?,
        Decl::Var(variable_declaration) => {
            validate_strict_mode_early_errors_in_variable_declaration(
                variable_declaration,
                strict,
            )?;
        }
        _ => {}
    }

    Ok(())
}

pub(super) fn validate_strict_mode_early_errors_in_function(
    function: &Function,
    strict: bool,
) -> Result<()> {
    let function_strict = strict || function_has_use_strict_directive(function);

    for parameter in &function.params {
        validate_strict_mode_early_errors_in_pattern(&parameter.pat, function_strict)?;
    }

    if let Some(body) = &function.body {
        validate_strict_mode_early_errors_in_statements(&body.stmts, function_strict)?;
    }

    Ok(())
}

pub(super) fn validate_strict_mode_early_errors_in_class(
    class: &Class,
    strict: bool,
) -> Result<()> {
    if let Some(super_class) = &class.super_class {
        validate_strict_mode_early_errors_in_expression(super_class, strict)?;
    }

    for member in &class.body {
        match member {
            ClassMember::Constructor(constructor) => {
                validate_strict_mode_early_errors_in_constructor(constructor, true)?;
            }
            ClassMember::Method(method) => {
                validate_property_name_strict_mode_early_errors(&method.key, true)?;
                validate_strict_mode_early_errors_in_function(&method.function, true)?;
            }
            ClassMember::ClassProp(property) => {
                validate_property_name_strict_mode_early_errors(&property.key, true)?;
                if let Some(value) = &property.value {
                    validate_strict_mode_early_errors_in_expression(value, true)?;
                }
            }
            ClassMember::PrivateMethod(method) => {
                validate_strict_mode_early_errors_in_function(&method.function, true)?;
            }
            ClassMember::PrivateProp(property) => {
                if let Some(value) = &property.value {
                    validate_strict_mode_early_errors_in_expression(value, true)?;
                }
            }
            ClassMember::StaticBlock(block) => {
                validate_strict_mode_early_errors_in_statements(&block.body.stmts, true)?;
            }
            _ => {}
        }
    }

    Ok(())
}

fn validate_strict_mode_early_errors_in_constructor(
    constructor: &Constructor,
    strict: bool,
) -> Result<()> {
    for parameter in &constructor.params {
        match parameter {
            ParamOrTsParamProp::Param(parameter) => {
                validate_strict_mode_early_errors_in_pattern(&parameter.pat, strict)?;
            }
            ParamOrTsParamProp::TsParamProp(_) => {}
        }
    }

    if let Some(body) = &constructor.body {
        validate_strict_mode_early_errors_in_statements(&body.stmts, strict)?;
    }

    Ok(())
}
