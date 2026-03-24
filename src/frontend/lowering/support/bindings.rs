use super::super::*;

pub(crate) fn collect_parameter_binding_names<'a>(
    parameters: impl IntoIterator<Item = &'a Pat>,
) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for parameter in parameters {
        collect_pattern_binding_names(parameter, &mut names)?;
    }
    Ok(names)
}

pub(crate) fn collect_function_scope_binding_names(statements: &[Stmt]) -> Result<Vec<String>> {
    fn collect_statement(statement: &Stmt, names: &mut Vec<String>) -> Result<()> {
        match statement {
            Stmt::Decl(Decl::Var(variable_declaration))
                if matches!(variable_declaration.kind, VarDeclKind::Var) =>
            {
                for declarator in &variable_declaration.decls {
                    collect_pattern_binding_names(&declarator.name, names)?;
                }
            }
            Stmt::Decl(Decl::Fn(function_declaration)) => {
                let name = function_declaration.ident.sym.to_string();
                if !names.contains(&name) {
                    names.push(name);
                }
            }
            Stmt::Block(block) => {
                for statement in &block.stmts {
                    collect_statement(statement, names)?;
                }
            }
            Stmt::Labeled(labeled_statement) => {
                collect_statement(&labeled_statement.body, names)?;
            }
            Stmt::If(if_statement) => {
                collect_statement(&if_statement.cons, names)?;
                if let Some(alternate) = &if_statement.alt {
                    collect_statement(alternate, names)?;
                }
            }
            Stmt::While(while_statement) => {
                collect_statement(&while_statement.body, names)?;
            }
            Stmt::DoWhile(do_while_statement) => {
                collect_statement(&do_while_statement.body, names)?;
            }
            Stmt::For(for_statement) => {
                if let Some(VarDeclOrExpr::VarDecl(variable_declaration)) = &for_statement.init
                    && matches!(variable_declaration.kind, VarDeclKind::Var)
                {
                    for declarator in &variable_declaration.decls {
                        collect_pattern_binding_names(&declarator.name, names)?;
                    }
                }
                collect_statement(&for_statement.body, names)?;
            }
            Stmt::ForIn(for_in_statement) => {
                if let ForHead::VarDecl(variable_declaration) = &for_in_statement.left
                    && matches!(variable_declaration.kind, VarDeclKind::Var)
                {
                    for declarator in &variable_declaration.decls {
                        collect_pattern_binding_names(&declarator.name, names)?;
                    }
                }
                collect_statement(&for_in_statement.body, names)?;
            }
            Stmt::ForOf(for_of_statement) => {
                if let ForHead::VarDecl(variable_declaration) = &for_of_statement.left
                    && matches!(variable_declaration.kind, VarDeclKind::Var)
                {
                    for declarator in &variable_declaration.decls {
                        collect_pattern_binding_names(&declarator.name, names)?;
                    }
                }
                collect_statement(&for_of_statement.body, names)?;
            }
            Stmt::Switch(switch_statement) => {
                for case in &switch_statement.cases {
                    for statement in &case.cons {
                        collect_statement(statement, names)?;
                    }
                }
            }
            Stmt::Try(try_statement) => {
                for statement in &try_statement.block.stmts {
                    collect_statement(statement, names)?;
                }
                if let Some(handler) = &try_statement.handler {
                    for statement in &handler.body.stmts {
                        collect_statement(statement, names)?;
                    }
                }
                if let Some(finalizer) = &try_statement.finalizer {
                    for statement in &finalizer.stmts {
                        collect_statement(statement, names)?;
                    }
                }
            }
            Stmt::With(with_statement) => {
                collect_statement(&with_statement.body, names)?;
            }
            _ => {}
        }

        Ok(())
    }

    let mut names = Vec::new();
    for statement in statements {
        collect_statement(statement, &mut names)?;
    }
    Ok(names)
}

pub(crate) fn collect_for_of_binding_names(pattern: &Pat, names: &mut Vec<String>) -> Result<()> {
    match pattern {
        Pat::Ident(identifier) => {
            let name = identifier.id.sym.to_string();
            if !names.contains(&name) {
                names.push(name);
            }
        }
        Pat::Assign(assign) => collect_for_of_binding_names(&assign.left, names)?,
        Pat::Array(array) => {
            for element in array.elems.iter().flatten() {
                collect_for_of_binding_names(element, names)?;
            }
        }
        _ => bail!("unsupported for-of binding pattern"),
    }

    Ok(())
}

pub(crate) fn collect_switch_bindings(switch_statement: &SwitchStmt) -> Result<Vec<String>> {
    let mut bindings = Vec::new();
    let mut seen = HashSet::new();

    for case in &switch_statement.cases {
        for statement in &case.cons {
            let Stmt::Decl(Decl::Var(variable_declaration)) = statement else {
                continue;
            };
            if matches!(variable_declaration.kind, VarDeclKind::Var) {
                continue;
            }

            for declarator in &variable_declaration.decls {
                let mut names = Vec::new();
                collect_pattern_binding_names(&declarator.name, &mut names)?;
                for name in names {
                    if seen.insert(name.clone()) {
                        bindings.push(name);
                    }
                }
            }
        }
    }

    Ok(bindings)
}

pub(crate) fn collect_direct_statement_lexical_bindings(
    statements: &[Stmt],
) -> Result<Vec<String>> {
    let mut bindings = Vec::new();

    for statement in statements {
        match statement {
            Stmt::Decl(Decl::Var(variable_declaration))
                if !matches!(variable_declaration.kind, VarDeclKind::Var) =>
            {
                for declarator in &variable_declaration.decls {
                    collect_pattern_binding_names(&declarator.name, &mut bindings)?;
                }
            }
            Stmt::Decl(Decl::Fn(function_declaration)) => {
                let name = function_declaration.ident.sym.to_string();
                if !bindings.contains(&name) {
                    bindings.push(name);
                }
            }
            Stmt::Decl(Decl::Class(class_declaration)) => {
                let name = class_declaration.ident.sym.to_string();
                if !bindings.contains(&name) {
                    bindings.push(name);
                }
            }
            _ => {}
        }
    }

    Ok(bindings)
}

pub(crate) fn collect_for_per_iteration_bindings(init: &VarDeclOrExpr) -> Result<Vec<String>> {
    let VarDeclOrExpr::VarDecl(variable_declaration) = init else {
        return Ok(Vec::new());
    };

    if matches!(variable_declaration.kind, VarDeclKind::Var) {
        return Ok(Vec::new());
    }

    Ok(variable_declaration
        .decls
        .iter()
        .map(|declarator| {
            let mut names = Vec::new();
            collect_pattern_binding_names(&declarator.name, &mut names)?;
            Ok(names)
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect())
}

pub(crate) fn binding_ident(pattern: &Pat) -> Result<&BindingIdent> {
    match pattern {
        Pat::Ident(identifier) => Ok(identifier),
        _ => bail!("only identifier bindings are supported"),
    }
}
