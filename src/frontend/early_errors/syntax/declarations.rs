use super::super::*;
use super::{
    expressions::validate_expression_syntax,
    functions::{validate_class_syntax, validate_function_syntax},
};

pub(crate) fn validate_declaration_syntax(
    declaration: &Decl,
    file: &swc_common::SourceFile,
) -> Result<()> {
    match declaration {
        Decl::Fn(function) => validate_function_syntax(&function.function, file)?,
        Decl::Class(class) => validate_class_syntax(&class.class, file)?,
        Decl::Var(variable_declaration) => {
            validate_variable_declaration_syntax(variable_declaration, file)?;
        }
        _ => {}
    }

    Ok(())
}

pub(super) fn validate_variable_declaration_syntax(
    declaration: &swc_ecma_ast::VarDecl,
    file: &swc_common::SourceFile,
) -> Result<()> {
    for declarator in &declaration.decls {
        validate_pattern_syntax(&declarator.name, file)?;
        if let Some(initializer) = &declarator.init {
            validate_expression_syntax(initializer, file)?;
        }
    }

    Ok(())
}

pub(super) fn validate_for_head_syntax(
    head: &ForHead,
    file: &swc_common::SourceFile,
) -> Result<()> {
    match head {
        ForHead::VarDecl(variable_declaration) => {
            validate_variable_declaration_syntax(variable_declaration, file)?;
        }
        ForHead::Pat(pattern) => validate_pattern_syntax(pattern, file)?,
        ForHead::UsingDecl(_) => {}
    }

    Ok(())
}

pub(super) fn validate_pattern_syntax(pattern: &Pat, file: &swc_common::SourceFile) -> Result<()> {
    match pattern {
        Pat::Assign(assign) => {
            validate_pattern_syntax(&assign.left, file)?;
            validate_expression_syntax(&assign.right, file)?;
        }
        Pat::Array(array) => {
            for element in array.elems.iter().flatten() {
                validate_pattern_syntax(element, file)?;
            }
        }
        Pat::Object(object) => {
            for property in &object.props {
                match property {
                    ObjectPatProp::KeyValue(property) => {
                        validate_pattern_syntax(&property.value, file)?;
                    }
                    ObjectPatProp::Assign(property) => {
                        if let Some(value) = &property.value {
                            validate_expression_syntax(value, file)?;
                        }
                    }
                    ObjectPatProp::Rest(rest) => validate_pattern_syntax(&rest.arg, file)?,
                }
            }
        }
        Pat::Rest(rest) => validate_pattern_syntax(&rest.arg, file)?,
        _ => {}
    }

    Ok(())
}
