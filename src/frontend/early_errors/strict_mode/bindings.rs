use super::super::*;
use super::{
    directives::is_strict_mode_restricted_identifier,
    expressions::validate_strict_mode_early_errors_in_expression,
};

pub(super) fn validate_strict_mode_early_errors_in_variable_declaration(
    declaration: &swc_ecma_ast::VarDecl,
    strict: bool,
) -> Result<()> {
    for declarator in &declaration.decls {
        validate_strict_mode_early_errors_in_pattern(&declarator.name, strict)?;
        if let Some(initializer) = &declarator.init {
            validate_strict_mode_early_errors_in_expression(initializer, strict)?;
        }
    }

    Ok(())
}

pub(super) fn validate_strict_mode_early_errors_in_for_head(
    head: &ForHead,
    strict: bool,
) -> Result<()> {
    match head {
        ForHead::VarDecl(variable_declaration) => {
            validate_strict_mode_early_errors_in_variable_declaration(
                variable_declaration,
                strict,
            )?;
        }
        ForHead::Pat(pattern) => validate_strict_mode_early_errors_in_pattern(pattern, strict)?,
        ForHead::UsingDecl(_) => {}
    }

    Ok(())
}

pub(super) fn validate_strict_mode_early_errors_in_pattern(
    pattern: &Pat,
    strict: bool,
) -> Result<()> {
    match pattern {
        Pat::Ident(identifier) => {
            ensure!(
                !strict || !is_strict_mode_restricted_identifier(identifier.id.sym.as_ref()),
                "strict mode forbids binding `{}`",
                identifier.id.sym
            );
        }
        Pat::Assign(assign) => {
            validate_strict_mode_early_errors_in_pattern(&assign.left, strict)?;
            validate_strict_mode_early_errors_in_expression(&assign.right, strict)?;
        }
        Pat::Array(array) => {
            for element in array.elems.iter().flatten() {
                validate_strict_mode_early_errors_in_pattern(element, strict)?;
            }
        }
        Pat::Object(object) => {
            for property in &object.props {
                match property {
                    ObjectPatProp::KeyValue(property) => {
                        validate_property_name_strict_mode_early_errors(&property.key, strict)?;
                        validate_strict_mode_early_errors_in_pattern(&property.value, strict)?;
                    }
                    ObjectPatProp::Assign(property) => {
                        ensure!(
                            !strict
                                || !is_strict_mode_restricted_identifier(property.key.sym.as_ref()),
                            "strict mode forbids binding `{}`",
                            property.key.sym
                        );
                        if let Some(value) = &property.value {
                            validate_strict_mode_early_errors_in_expression(value, strict)?;
                        }
                    }
                    ObjectPatProp::Rest(rest) => {
                        validate_strict_mode_early_errors_in_pattern(&rest.arg, strict)?;
                    }
                }
            }
        }
        Pat::Rest(rest) => validate_strict_mode_early_errors_in_pattern(&rest.arg, strict)?,
        _ => {}
    }

    Ok(())
}

pub(super) fn validate_property_name_strict_mode_early_errors(
    name: &PropName,
    strict: bool,
) -> Result<()> {
    if let PropName::Computed(computed) = name {
        validate_strict_mode_early_errors_in_expression(&computed.expr, strict)?;
    }

    Ok(())
}

pub(super) fn validate_strict_mode_assignment_target(
    target: &AssignTarget,
    strict: bool,
) -> Result<()> {
    if strict && let AssignTarget::Simple(SimpleAssignTarget::Ident(identifier)) = target {
        ensure!(
            !is_strict_mode_restricted_identifier(identifier.id.sym.as_ref()),
            "strict mode forbids assigning to `{}`",
            identifier.id.sym
        );
    }

    match target {
        AssignTarget::Simple(SimpleAssignTarget::Member(member)) => {
            validate_strict_mode_early_errors_in_expression(&member.obj, strict)?;
            if let MemberProp::Computed(property) = &member.prop {
                validate_strict_mode_early_errors_in_expression(&property.expr, strict)?;
            }
        }
        AssignTarget::Pat(_) | AssignTarget::Simple(_) => {}
    }

    Ok(())
}
