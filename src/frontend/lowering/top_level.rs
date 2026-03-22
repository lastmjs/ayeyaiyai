use super::*;

impl Lowerer {
    pub(crate) fn lower_top_level_statements<'a>(
        &mut self,
        statements: impl Iterator<Item = &'a Stmt>,
        lowered_statements: &mut Vec<Statement>,
    ) -> Result<()> {
        for statement in statements {
            self.lower_top_level_statement(statement, lowered_statements)?;
        }

        Ok(())
    }

    pub(crate) fn lower_top_level_statement(
        &mut self,
        statement: &Stmt,
        lowered_statements: &mut Vec<Statement>,
    ) -> Result<()> {
        match statement {
            Stmt::Decl(Decl::Fn(function_declaration)) => {
                if self.module_mode {
                    lowered_statements
                        .extend(self.lower_nested_function_declaration(function_declaration)?);
                } else {
                    let lowered = self.lower_function_declaration(function_declaration, true)?;
                    self.functions.push(lowered);
                }
            }
            other => lowered_statements.extend(self.lower_statement(other, false, false)?),
        }

        Ok(())
    }

    pub(crate) fn lower_module_declaration(
        &mut self,
        module_declaration: &ModuleDecl,
        lowered_statements: &mut Vec<Statement>,
    ) -> Result<()> {
        match module_declaration {
            ModuleDecl::ExportDecl(export) => match &export.decl {
                Decl::Fn(function_declaration) => lowered_statements
                    .extend(self.lower_nested_function_declaration(function_declaration)?),
                Decl::Var(variable_declaration) => lowered_statements
                    .extend(self.lower_variable_declaration(variable_declaration)?),
                other => bail!("unsupported export declaration: {other:?}"),
            },
            ModuleDecl::ExportDefaultDecl(export_default) => {
                lowered_statements.extend(self.lower_export_default_declaration(export_default)?)
            }
            ModuleDecl::ExportDefaultExpr(export_default) => {
                lowered_statements.push(Statement::Expression(
                    self.lower_expression_with_name_hint(&export_default.expr, Some("default"))?,
                ));
            }
            ModuleDecl::ExportNamed(export_named) if export_named.src.is_none() => {}
            ModuleDecl::Import(_) | ModuleDecl::ExportNamed(_) | ModuleDecl::ExportAll(_) => {
                bail!("import and export declarations are not supported yet")
            }
            other => bail!("unsupported module declaration: {other:?}"),
        }

        Ok(())
    }

    pub(crate) fn lower_export_default_declaration(
        &mut self,
        export_default: &ExportDefaultDecl,
    ) -> Result<Vec<Statement>> {
        match &export_default.decl {
            DefaultDecl::Fn(function_expression) => {
                if let Some(identifier) = &function_expression.ident {
                    let generated_name =
                        self.lower_named_default_function_expression(function_expression)?;
                    Ok(vec![Statement::Let {
                        name: identifier.sym.to_string(),
                        mutable: true,
                        value: Expression::Identifier(generated_name),
                    }])
                } else {
                    Ok(vec![Statement::Expression(
                        self.lower_function_expression(function_expression, Some("default"))?,
                    )])
                }
            }
            other => bail!("unsupported default export declaration: {other:?}"),
        }
    }
}
