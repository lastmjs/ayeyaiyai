use super::super::*;
use super::{
    blocks::{validate_block_statement_early_errors, validate_classic_for_header},
    declarations::{
        validate_declaration_syntax, validate_for_head_syntax, validate_pattern_syntax,
        validate_variable_declaration_syntax,
    },
    expressions::validate_expression_syntax,
};

pub(crate) fn validate_statement_syntax(
    statement: &Stmt,
    file: &swc_common::SourceFile,
) -> Result<()> {
    match statement {
        Stmt::Block(block) => {
            validate_block_statement_early_errors(&block.stmts)?;
            for statement in &block.stmts {
                validate_statement_syntax(statement, file)?;
            }
        }
        Stmt::Decl(declaration) => validate_declaration_syntax(declaration, file)?,
        Stmt::Expr(expression) => validate_expression_syntax(&expression.expr, file)?,
        Stmt::If(statement) => {
            validate_expression_syntax(&statement.test, file)?;
            validate_statement_syntax(&statement.cons, file)?;
            if let Some(alternate) = &statement.alt {
                validate_statement_syntax(alternate, file)?;
            }
        }
        Stmt::While(statement) => {
            validate_expression_syntax(&statement.test, file)?;
            validate_statement_syntax(&statement.body, file)?;
        }
        Stmt::DoWhile(statement) => {
            validate_statement_syntax(&statement.body, file)?;
            validate_expression_syntax(&statement.test, file)?;
        }
        Stmt::For(statement) => {
            validate_classic_for_header(statement, file)?;
            if let Some(init) = &statement.init {
                match init {
                    VarDeclOrExpr::VarDecl(variable_declaration) => {
                        validate_variable_declaration_syntax(variable_declaration, file)?;
                    }
                    VarDeclOrExpr::Expr(expression) => {
                        validate_expression_syntax(expression, file)?
                    }
                }
            }
            if let Some(test) = &statement.test {
                validate_expression_syntax(test, file)?;
            }
            if let Some(update) = &statement.update {
                validate_expression_syntax(update, file)?;
            }
            validate_statement_syntax(&statement.body, file)?;
        }
        Stmt::ForIn(statement) => {
            validate_for_head_syntax(&statement.left, file)?;
            validate_expression_syntax(&statement.right, file)?;
            validate_statement_syntax(&statement.body, file)?;
        }
        Stmt::ForOf(statement) => {
            validate_for_head_syntax(&statement.left, file)?;
            validate_expression_syntax(&statement.right, file)?;
            validate_statement_syntax(&statement.body, file)?;
        }
        Stmt::Switch(statement) => {
            validate_expression_syntax(&statement.discriminant, file)?;
            for case in &statement.cases {
                if let Some(test) = &case.test {
                    validate_expression_syntax(test, file)?;
                }
                for statement in &case.cons {
                    validate_statement_syntax(statement, file)?;
                }
            }
        }
        Stmt::Try(statement) => {
            for statement in &statement.block.stmts {
                validate_statement_syntax(statement, file)?;
            }
            if let Some(handler) = &statement.handler {
                if let Some(pattern) = &handler.param {
                    validate_pattern_syntax(pattern, file)?;
                }
                for statement in &handler.body.stmts {
                    validate_statement_syntax(statement, file)?;
                }
            }
            if let Some(finalizer) = &statement.finalizer {
                for statement in &finalizer.stmts {
                    validate_statement_syntax(statement, file)?;
                }
            }
        }
        Stmt::With(statement) => {
            validate_expression_syntax(&statement.obj, file)?;
            validate_statement_syntax(&statement.body, file)?;
        }
        Stmt::Return(statement) => {
            if let Some(argument) = &statement.arg {
                validate_expression_syntax(argument, file)?;
            }
        }
        Stmt::Throw(statement) => validate_expression_syntax(&statement.arg, file)?,
        Stmt::Labeled(statement) => validate_statement_syntax(&statement.body, file)?,
        _ => {}
    }

    Ok(())
}
