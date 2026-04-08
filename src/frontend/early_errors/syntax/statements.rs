use super::super::*;
use super::{
    blocks::{validate_block_statement_early_errors, validate_classic_for_header},
    declarations::{
        BindingRestrictions, is_await_like_identifier, is_yield_like_identifier,
        validate_declaration_syntax, validate_for_head_syntax_with_restrictions,
        validate_pattern_syntax_with_restrictions,
        validate_variable_declaration_syntax_with_restrictions,
    },
    expressions::validate_expression_syntax_with_restrictions,
};

pub(crate) fn validate_statement_syntax(
    statement: &Stmt,
    file: &swc_common::SourceFile,
) -> Result<()> {
    validate_statement_syntax_with_restrictions(statement, file, BindingRestrictions::default())
}

pub(super) fn validate_statement_syntax_with_restrictions(
    statement: &Stmt,
    file: &swc_common::SourceFile,
    restrictions: BindingRestrictions,
) -> Result<()> {
    match statement {
        Stmt::Block(block) => {
            validate_block_statement_early_errors(&block.stmts)?;
            for statement in &block.stmts {
                validate_statement_syntax_with_restrictions(statement, file, restrictions)?;
            }
        }
        Stmt::Decl(declaration) => match declaration {
            Decl::Var(variable_declaration) => {
                validate_variable_declaration_syntax_with_restrictions(
                    variable_declaration,
                    file,
                    restrictions,
                )?
            }
            _ => validate_declaration_syntax(declaration, file)?,
        },
        Stmt::Expr(expression) => {
            validate_expression_syntax_with_restrictions(&expression.expr, file, restrictions)?
        }
        Stmt::If(statement) => {
            validate_expression_syntax_with_restrictions(&statement.test, file, restrictions)?;
            validate_statement_syntax_with_restrictions(&statement.cons, file, restrictions)?;
            if let Some(alternate) = &statement.alt {
                validate_statement_syntax_with_restrictions(alternate, file, restrictions)?;
            }
        }
        Stmt::While(statement) => {
            validate_expression_syntax_with_restrictions(&statement.test, file, restrictions)?;
            validate_statement_syntax_with_restrictions(&statement.body, file, restrictions)?;
        }
        Stmt::DoWhile(statement) => {
            validate_statement_syntax_with_restrictions(&statement.body, file, restrictions)?;
            validate_expression_syntax_with_restrictions(&statement.test, file, restrictions)?;
        }
        Stmt::For(statement) => {
            validate_classic_for_header(statement, file)?;
            if let Some(init) = &statement.init {
                match init {
                    VarDeclOrExpr::VarDecl(variable_declaration) => {
                        validate_variable_declaration_syntax_with_restrictions(
                            variable_declaration,
                            file,
                            restrictions,
                        )?;
                    }
                    VarDeclOrExpr::Expr(expression) => {
                        validate_expression_syntax_with_restrictions(
                            expression,
                            file,
                            restrictions,
                        )?
                    }
                }
            }
            if let Some(test) = &statement.test {
                validate_expression_syntax_with_restrictions(test, file, restrictions)?;
            }
            if let Some(update) = &statement.update {
                validate_expression_syntax_with_restrictions(update, file, restrictions)?;
            }
            validate_statement_syntax_with_restrictions(&statement.body, file, restrictions)?;
        }
        Stmt::ForIn(statement) => {
            validate_for_head_syntax_with_restrictions(&statement.left, file, restrictions)?;
            validate_expression_syntax_with_restrictions(&statement.right, file, restrictions)?;
            validate_statement_syntax_with_restrictions(&statement.body, file, restrictions)?;
        }
        Stmt::ForOf(statement) => {
            validate_for_head_syntax_with_restrictions(&statement.left, file, restrictions)?;
            validate_expression_syntax_with_restrictions(&statement.right, file, restrictions)?;
            validate_statement_syntax_with_restrictions(&statement.body, file, restrictions)?;
        }
        Stmt::Switch(statement) => {
            validate_expression_syntax_with_restrictions(
                &statement.discriminant,
                file,
                restrictions,
            )?;
            for case in &statement.cases {
                if let Some(test) = &case.test {
                    validate_expression_syntax_with_restrictions(test, file, restrictions)?;
                }
                for statement in &case.cons {
                    validate_statement_syntax_with_restrictions(statement, file, restrictions)?;
                }
            }
        }
        Stmt::Try(statement) => {
            for statement in &statement.block.stmts {
                validate_statement_syntax_with_restrictions(statement, file, restrictions)?;
            }
            if let Some(handler) = &statement.handler {
                if let Some(pattern) = &handler.param {
                    validate_pattern_syntax_with_restrictions(pattern, file, restrictions)?;
                }
                for statement in &handler.body.stmts {
                    validate_statement_syntax_with_restrictions(statement, file, restrictions)?;
                }
            }
            if let Some(finalizer) = &statement.finalizer {
                for statement in &finalizer.stmts {
                    validate_statement_syntax_with_restrictions(statement, file, restrictions)?;
                }
            }
        }
        Stmt::With(statement) => {
            validate_expression_syntax_with_restrictions(&statement.obj, file, restrictions)?;
            validate_statement_syntax_with_restrictions(&statement.body, file, restrictions)?;
        }
        Stmt::Return(statement) => {
            if let Some(argument) = &statement.arg {
                validate_expression_syntax_with_restrictions(argument, file, restrictions)?;
            }
        }
        Stmt::Throw(statement) => {
            validate_expression_syntax_with_restrictions(&statement.arg, file, restrictions)?
        }
        Stmt::Labeled(statement) => {
            ensure!(
                !(restrictions.await_reserved
                    && is_await_like_identifier(statement.label.sym.as_ref())),
                "`await` cannot be used as a label in an async function"
            );
            ensure!(
                !(restrictions.yield_reserved
                    && is_yield_like_identifier(statement.label.sym.as_ref())),
                "`yield` cannot be used as a label in a generator function"
            );
            validate_statement_syntax_with_restrictions(&statement.body, file, restrictions)?
        }
        _ => {}
    }

    Ok(())
}
