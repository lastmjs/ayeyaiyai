use super::super::*;
use super::{
    bindings::{
        validate_property_name_strict_mode_early_errors, validate_strict_mode_assignment_target,
        validate_strict_mode_early_errors_in_pattern,
    },
    directives::is_strict_mode_restricted_identifier,
    functions::{
        validate_strict_mode_early_errors_in_class, validate_strict_mode_early_errors_in_function,
    },
    statements::validate_strict_mode_early_errors_in_statements,
};

pub(super) fn validate_strict_mode_early_errors_in_expression(
    expression: &Expr,
    strict: bool,
) -> Result<()> {
    match expression {
        Expr::Call(call) => {
            if let Callee::Expr(callee) = &call.callee {
                validate_strict_mode_early_errors_in_expression(callee, strict)?;
            }
            for argument in &call.args {
                validate_strict_mode_early_errors_in_expression(&argument.expr, strict)?;
            }
        }
        Expr::New(new_expression) => {
            validate_strict_mode_early_errors_in_expression(&new_expression.callee, strict)?;
            for argument in new_expression.args.iter().flatten() {
                validate_strict_mode_early_errors_in_expression(&argument.expr, strict)?;
            }
        }
        Expr::Await(await_expression) => {
            validate_strict_mode_early_errors_in_expression(&await_expression.arg, strict)?;
        }
        Expr::Yield(yield_expression) => {
            if let Some(argument) = &yield_expression.arg {
                validate_strict_mode_early_errors_in_expression(argument, strict)?;
            }
        }
        Expr::Paren(parenthesized) => {
            validate_strict_mode_early_errors_in_expression(&parenthesized.expr, strict)?;
        }
        Expr::Array(array) => {
            for element in array.elems.iter().flatten() {
                validate_strict_mode_early_errors_in_expression(&element.expr, strict)?;
            }
        }
        Expr::Object(object) => {
            for property in &object.props {
                match property {
                    PropOrSpread::Spread(spread) => {
                        validate_strict_mode_early_errors_in_expression(&spread.expr, strict)?;
                    }
                    PropOrSpread::Prop(property) => match &**property {
                        Prop::Shorthand(identifier) => {
                            ensure!(
                                !strict
                                    || !is_strict_mode_restricted_identifier(
                                        identifier.sym.as_ref()
                                    ),
                                "strict mode forbids binding `{}`",
                                identifier.sym
                            );
                        }
                        Prop::KeyValue(property) => {
                            validate_property_name_strict_mode_early_errors(&property.key, strict)?;
                            validate_strict_mode_early_errors_in_expression(
                                &property.value,
                                strict,
                            )?;
                        }
                        Prop::Getter(property) => {
                            validate_property_name_strict_mode_early_errors(&property.key, strict)?;
                            if let Some(body) = &property.body {
                                validate_strict_mode_early_errors_in_statements(
                                    &body.stmts,
                                    strict,
                                )?;
                            }
                        }
                        Prop::Setter(property) => {
                            validate_property_name_strict_mode_early_errors(&property.key, strict)?;
                            validate_strict_mode_early_errors_in_pattern(&property.param, strict)?;
                            if let Some(body) = &property.body {
                                validate_strict_mode_early_errors_in_statements(
                                    &body.stmts,
                                    strict,
                                )?;
                            }
                        }
                        Prop::Method(property) => {
                            validate_property_name_strict_mode_early_errors(&property.key, strict)?;
                            validate_strict_mode_early_errors_in_function(
                                &property.function,
                                strict,
                            )?;
                        }
                        Prop::Assign(property) => {
                            ensure!(
                                !strict
                                    || !is_strict_mode_restricted_identifier(
                                        property.key.sym.as_ref()
                                    ),
                                "strict mode forbids binding `{}`",
                                property.key.sym
                            );
                            validate_strict_mode_early_errors_in_expression(
                                &property.value,
                                strict,
                            )?;
                        }
                    },
                }
            }
        }
        Expr::Member(member) => {
            validate_strict_mode_early_errors_in_expression(&member.obj, strict)?;
            if let MemberProp::Computed(property) = &member.prop {
                validate_strict_mode_early_errors_in_expression(&property.expr, strict)?;
            }
        }
        Expr::Unary(unary) => {
            if strict && unary.op == SwcUnaryOp::Delete && matches!(&*unary.arg, Expr::Ident(_)) {
                bail!("strict mode forbids deleting unqualified identifiers");
            }
            validate_strict_mode_early_errors_in_expression(&unary.arg, strict)?;
        }
        Expr::Update(update) => {
            if strict && let Expr::Ident(identifier) = &*update.arg {
                ensure!(
                    !is_strict_mode_restricted_identifier(identifier.sym.as_ref()),
                    "strict mode forbids updating `{}`",
                    identifier.sym
                );
            }
            validate_strict_mode_early_errors_in_expression(&update.arg, strict)?;
        }
        Expr::Bin(binary) => {
            validate_strict_mode_early_errors_in_expression(&binary.left, strict)?;
            validate_strict_mode_early_errors_in_expression(&binary.right, strict)?;
        }
        Expr::Assign(assignment) => {
            validate_strict_mode_assignment_target(&assignment.left, strict)?;
            validate_strict_mode_early_errors_in_expression(&assignment.right, strict)?;
        }
        Expr::Cond(conditional) => {
            validate_strict_mode_early_errors_in_expression(&conditional.test, strict)?;
            validate_strict_mode_early_errors_in_expression(&conditional.cons, strict)?;
            validate_strict_mode_early_errors_in_expression(&conditional.alt, strict)?;
        }
        Expr::Seq(sequence) => {
            for expression in &sequence.exprs {
                validate_strict_mode_early_errors_in_expression(expression, strict)?;
            }
        }
        Expr::Fn(function) => {
            if let Some(identifier) = &function.ident {
                ensure!(
                    !strict || !is_strict_mode_restricted_identifier(identifier.sym.as_ref()),
                    "strict mode forbids binding `{}`",
                    identifier.sym
                );
            }
            validate_strict_mode_early_errors_in_function(&function.function, strict)?;
        }
        Expr::Arrow(arrow) => {
            for parameter in &arrow.params {
                validate_strict_mode_early_errors_in_pattern(parameter, strict)?;
            }
            match &*arrow.body {
                BlockStmtOrExpr::BlockStmt(block) => {
                    validate_strict_mode_early_errors_in_statements(&block.stmts, strict)?;
                }
                BlockStmtOrExpr::Expr(expression) => {
                    validate_strict_mode_early_errors_in_expression(expression, strict)?;
                }
            }
        }
        Expr::Class(class) => {
            if let Some(identifier) = &class.ident {
                ensure!(
                    !strict || !is_strict_mode_restricted_identifier(identifier.sym.as_ref()),
                    "strict mode forbids binding `{}`",
                    identifier.sym
                );
            }
            validate_strict_mode_early_errors_in_class(&class.class, strict)?;
        }
        Expr::Tpl(template) => {
            for expression in &template.exprs {
                validate_strict_mode_early_errors_in_expression(expression, strict)?;
            }
        }
        Expr::TaggedTpl(tagged) => {
            validate_strict_mode_early_errors_in_expression(&tagged.tag, strict)?;
            for expression in &tagged.tpl.exprs {
                validate_strict_mode_early_errors_in_expression(expression, strict)?;
            }
        }
        _ => {}
    }

    Ok(())
}
