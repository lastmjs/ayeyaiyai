use super::super::*;
use super::{
    declarations::validate_pattern_syntax,
    functions::{
        ensure_parameter_names_are_valid, validate_class_syntax, validate_function_syntax,
        validate_property_name_syntax,
    },
    statements::validate_statement_syntax,
};

pub(crate) fn validate_expression_syntax(
    expression: &Expr,
    file: &swc_common::SourceFile,
) -> Result<()> {
    match expression {
        Expr::Call(call) => {
            if let Callee::Expr(callee) = &call.callee {
                validate_expression_syntax(callee, file)?;
            }
            for argument in &call.args {
                validate_expression_syntax(&argument.expr, file)?;
            }
        }
        Expr::New(new_expression) => {
            validate_expression_syntax(&new_expression.callee, file)?;
            for argument in new_expression.args.iter().flatten() {
                validate_expression_syntax(&argument.expr, file)?;
            }
        }
        Expr::Await(await_expression) => {
            validate_expression_syntax(&await_expression.arg, file)?;
        }
        Expr::Yield(yield_expression) => {
            if let Some(argument) = &yield_expression.arg {
                validate_expression_syntax(argument, file)?;
            }
        }
        Expr::Paren(parenthesized) => validate_expression_syntax(&parenthesized.expr, file)?,
        Expr::Array(array) => {
            for element in array.elems.iter().flatten() {
                validate_expression_syntax(&element.expr, file)?;
            }
        }
        Expr::Object(object) => {
            for property in &object.props {
                match property {
                    PropOrSpread::Spread(spread) => validate_expression_syntax(&spread.expr, file)?,
                    PropOrSpread::Prop(property) => match &**property {
                        Prop::Shorthand(_) => {}
                        Prop::KeyValue(property) => {
                            validate_property_name_syntax(&property.key, file)?;
                            validate_expression_syntax(&property.value, file)?;
                        }
                        Prop::Getter(property) => {
                            validate_property_name_syntax(&property.key, file)?;
                            if let Some(body) = &property.body {
                                for statement in &body.stmts {
                                    validate_statement_syntax(statement, file)?;
                                }
                            }
                        }
                        Prop::Setter(property) => {
                            validate_property_name_syntax(&property.key, file)?;
                            validate_pattern_syntax(&property.param, file)?;
                            if let Some(body) = &property.body {
                                for statement in &body.stmts {
                                    validate_statement_syntax(statement, file)?;
                                }
                            }
                        }
                        Prop::Method(property) => {
                            validate_property_name_syntax(&property.key, file)?;
                            validate_function_syntax(&property.function, file)?;
                        }
                        Prop::Assign(property) => {
                            validate_expression_syntax(&property.value, file)?
                        }
                    },
                }
            }
        }
        Expr::Member(member) => {
            validate_expression_syntax(&member.obj, file)?;
            if let MemberProp::Computed(property) = &member.prop {
                validate_expression_syntax(&property.expr, file)?;
            }
        }
        Expr::Unary(unary) => validate_expression_syntax(&unary.arg, file)?,
        Expr::Update(update) => validate_expression_syntax(&update.arg, file)?,
        Expr::Bin(binary) => {
            validate_expression_syntax(&binary.left, file)?;
            validate_expression_syntax(&binary.right, file)?;
        }
        Expr::Assign(assignment) => {
            match &assignment.left {
                AssignTarget::Simple(SimpleAssignTarget::Member(member)) => {
                    validate_expression_syntax(&member.obj, file)?;
                    if let MemberProp::Computed(property) = &member.prop {
                        validate_expression_syntax(&property.expr, file)?;
                    }
                }
                AssignTarget::Simple(_) | AssignTarget::Pat(_) => {}
            }
            validate_expression_syntax(&assignment.right, file)?;
        }
        Expr::Cond(conditional) => {
            validate_expression_syntax(&conditional.test, file)?;
            validate_expression_syntax(&conditional.cons, file)?;
            validate_expression_syntax(&conditional.alt, file)?;
        }
        Expr::Seq(sequence) => {
            for expression in &sequence.exprs {
                validate_expression_syntax(expression, file)?;
            }
        }
        Expr::Fn(function) => validate_function_syntax(&function.function, file)?,
        Expr::Arrow(arrow) => {
            ensure_parameter_names_are_valid(
                arrow.params.iter(),
                arrow
                    .params
                    .iter()
                    .all(|parameter| matches!(parameter, Pat::Ident(_))),
                false,
            )?;
            for parameter in &arrow.params {
                validate_pattern_syntax(parameter, file)?;
            }
            match &*arrow.body {
                BlockStmtOrExpr::BlockStmt(block) => {
                    for statement in &block.stmts {
                        validate_statement_syntax(statement, file)?;
                    }
                }
                BlockStmtOrExpr::Expr(expression) => validate_expression_syntax(expression, file)?,
            }
        }
        Expr::Class(class) => validate_class_syntax(&class.class, file)?,
        Expr::Tpl(template) => {
            for expression in &template.exprs {
                validate_expression_syntax(expression, file)?;
            }
        }
        Expr::TaggedTpl(tagged) => {
            validate_expression_syntax(&tagged.tag, file)?;
            for expression in &tagged.tpl.exprs {
                validate_expression_syntax(expression, file)?;
            }
        }
        _ => {}
    }

    Ok(())
}
