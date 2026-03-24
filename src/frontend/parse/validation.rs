use anyhow::Result;
use swc_ecma_ast::{DefaultDecl, ExportDefaultDecl, Module, ModuleDecl, ModuleItem};

use crate::frontend::early_errors::{
    script_has_use_strict_directive, validate_class_syntax, validate_declaration_syntax,
    validate_expression_syntax, validate_function_syntax, validate_import_attributes,
    validate_statement_syntax, validate_strict_mode_early_errors_in_module_items,
    validate_strict_mode_early_errors_in_statements,
};

pub(super) fn validate_script_ast(
    script: &swc_ecma_ast::Script,
    file: &swc_common::SourceFile,
) -> Result<()> {
    for statement in &script.body {
        validate_statement_syntax(statement, file)?;
    }

    validate_strict_mode_early_errors_in_statements(
        &script.body,
        script_has_use_strict_directive(&script.body),
    )?;

    Ok(())
}

pub(super) fn validate_module_ast(module: &Module, file: &swc_common::SourceFile) -> Result<()> {
    for item in &module.body {
        match item {
            ModuleItem::Stmt(statement) => validate_statement_syntax(statement, file)?,
            ModuleItem::ModuleDecl(module_declaration) => match module_declaration {
                ModuleDecl::Import(import) => {
                    validate_import_attributes(import.with.as_deref())?;
                }
                ModuleDecl::ExportNamed(export) => {
                    validate_import_attributes(export.with.as_deref())?;
                }
                ModuleDecl::ExportAll(export) => {
                    validate_import_attributes(export.with.as_deref())?;
                }
                ModuleDecl::ExportDecl(export) => validate_declaration_syntax(&export.decl, file)?,
                ModuleDecl::ExportDefaultDecl(ExportDefaultDecl { decl, .. }) => match decl {
                    DefaultDecl::Fn(function) => {
                        validate_function_syntax(&function.function, file)?
                    }
                    DefaultDecl::Class(class) => validate_class_syntax(&class.class, file)?,
                    _ => {}
                },
                ModuleDecl::ExportDefaultExpr(export) => {
                    validate_expression_syntax(&export.expr, file)?;
                }
                _ => {}
            },
        }
    }

    validate_strict_mode_early_errors_in_module_items(&module.body, true)?;

    Ok(())
}
