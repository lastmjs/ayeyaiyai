use super::super::*;
use super::{
    bindings::{
        validate_strict_mode_early_errors_in_for_head,
        validate_strict_mode_early_errors_in_pattern,
        validate_strict_mode_early_errors_in_variable_declaration,
    },
    expressions::validate_strict_mode_early_errors_in_expression,
    functions::validate_strict_mode_early_errors_in_declaration,
};

pub(crate) fn validate_strict_mode_early_errors_in_module_items(
    items: &[ModuleItem],
    strict: bool,
) -> Result<()> {
    for item in items {
        match item {
            ModuleItem::Stmt(statement) => {
                validate_strict_mode_early_errors_in_statement(statement, strict)?;
            }
            ModuleItem::ModuleDecl(module_declaration) => match module_declaration {
                ModuleDecl::ExportDecl(export) => {
                    validate_strict_mode_early_errors_in_declaration(&export.decl, strict)?;
                }
                ModuleDecl::ExportDefaultDecl(ExportDefaultDecl { decl, .. }) => match decl {
                    DefaultDecl::Fn(function) => {
                        super::functions::validate_strict_mode_early_errors_in_function(
                            &function.function,
                            strict,
                        )?;
                    }
                    DefaultDecl::Class(class) => {
                        super::functions::validate_strict_mode_early_errors_in_class(
                            &class.class,
                            strict,
                        )?;
                    }
                    _ => {}
                },
                ModuleDecl::ExportDefaultExpr(export) => {
                    validate_strict_mode_early_errors_in_expression(&export.expr, strict)?;
                }
                _ => {}
            },
        }
    }

    Ok(())
}

pub(crate) fn validate_strict_mode_early_errors_in_statements(
    statements: &[Stmt],
    strict: bool,
) -> Result<()> {
    for statement in statements {
        validate_strict_mode_early_errors_in_statement(statement, strict)?;
    }

    Ok(())
}

fn validate_strict_mode_early_errors_in_statement(statement: &Stmt, strict: bool) -> Result<()> {
    match statement {
        Stmt::Block(block) => {
            validate_strict_mode_early_errors_in_statements(&block.stmts, strict)?;
        }
        Stmt::Decl(declaration) => {
            validate_strict_mode_early_errors_in_declaration(declaration, strict)?;
        }
        Stmt::Expr(expression) => {
            validate_strict_mode_early_errors_in_expression(&expression.expr, strict)?;
        }
        Stmt::If(statement) => {
            validate_strict_mode_early_errors_in_expression(&statement.test, strict)?;
            validate_strict_mode_early_errors_in_statement(&statement.cons, strict)?;
            if let Some(alternate) = &statement.alt {
                validate_strict_mode_early_errors_in_statement(alternate, strict)?;
            }
        }
        Stmt::While(statement) => {
            validate_strict_mode_early_errors_in_expression(&statement.test, strict)?;
            validate_strict_mode_early_errors_in_statement(&statement.body, strict)?;
        }
        Stmt::DoWhile(statement) => {
            validate_strict_mode_early_errors_in_statement(&statement.body, strict)?;
            validate_strict_mode_early_errors_in_expression(&statement.test, strict)?;
        }
        Stmt::For(statement) => {
            if let Some(init) = &statement.init {
                match init {
                    VarDeclOrExpr::VarDecl(variable_declaration) => {
                        validate_strict_mode_early_errors_in_variable_declaration(
                            variable_declaration,
                            strict,
                        )?;
                    }
                    VarDeclOrExpr::Expr(expression) => {
                        validate_strict_mode_early_errors_in_expression(expression, strict)?;
                    }
                }
            }
            if let Some(test) = &statement.test {
                validate_strict_mode_early_errors_in_expression(test, strict)?;
            }
            if let Some(update) = &statement.update {
                validate_strict_mode_early_errors_in_expression(update, strict)?;
            }
            validate_strict_mode_early_errors_in_statement(&statement.body, strict)?;
        }
        Stmt::ForIn(statement) => {
            validate_strict_mode_early_errors_in_for_head(&statement.left, strict)?;
            validate_strict_mode_early_errors_in_expression(&statement.right, strict)?;
            validate_strict_mode_early_errors_in_statement(&statement.body, strict)?;
        }
        Stmt::ForOf(statement) => {
            validate_strict_mode_early_errors_in_for_head(&statement.left, strict)?;
            validate_strict_mode_early_errors_in_expression(&statement.right, strict)?;
            validate_strict_mode_early_errors_in_statement(&statement.body, strict)?;
        }
        Stmt::Switch(statement) => {
            validate_strict_mode_early_errors_in_expression(&statement.discriminant, strict)?;
            for case in &statement.cases {
                if let Some(test) = &case.test {
                    validate_strict_mode_early_errors_in_expression(test, strict)?;
                }
                validate_strict_mode_early_errors_in_statements(&case.cons, strict)?;
            }
        }
        Stmt::Try(statement) => {
            validate_strict_mode_early_errors_in_statements(&statement.block.stmts, strict)?;
            if let Some(handler) = &statement.handler {
                if let Some(pattern) = &handler.param {
                    validate_strict_mode_early_errors_in_pattern(pattern, strict)?;
                }
                validate_strict_mode_early_errors_in_statements(&handler.body.stmts, strict)?;
            }
            if let Some(finalizer) = &statement.finalizer {
                validate_strict_mode_early_errors_in_statements(&finalizer.stmts, strict)?;
            }
        }
        Stmt::With(statement) => {
            validate_strict_mode_early_errors_in_expression(&statement.obj, strict)?;
            validate_strict_mode_early_errors_in_statement(&statement.body, strict)?;
        }
        Stmt::Return(statement) => {
            if let Some(argument) = &statement.arg {
                validate_strict_mode_early_errors_in_expression(argument, strict)?;
            }
        }
        Stmt::Throw(statement) => {
            validate_strict_mode_early_errors_in_expression(&statement.arg, strict)?;
        }
        Stmt::Labeled(statement) => {
            validate_strict_mode_early_errors_in_statement(&statement.body, strict)?;
        }
        _ => {}
    }

    Ok(())
}
