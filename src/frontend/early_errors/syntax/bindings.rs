use super::super::*;

pub(crate) fn collect_var_decl_bound_names(
    variable_declaration: &swc_ecma_ast::VarDecl,
) -> Result<Vec<String>> {
    let mut names = Vec::new();

    for declarator in &variable_declaration.decls {
        collect_pattern_binding_names(&declarator.name, &mut names)?;
    }

    Ok(names)
}

pub(crate) fn collect_module_declared_names(module: &Module) -> Result<HashSet<String>> {
    let mut names = HashSet::new();

    for item in &module.body {
        match item {
            ModuleItem::Stmt(Stmt::Decl(Decl::Fn(function_declaration))) => {
                names.insert(function_declaration.ident.sym.to_string());
            }
            ModuleItem::Stmt(Stmt::Decl(Decl::Class(class_declaration))) => {
                names.insert(class_declaration.ident.sym.to_string());
            }
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(variable_declaration))) => {
                names.extend(collect_var_decl_bound_names(variable_declaration)?);
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => match &export.decl {
                Decl::Fn(function_declaration) => {
                    names.insert(function_declaration.ident.sym.to_string());
                }
                Decl::Class(class_declaration) => {
                    names.insert(class_declaration.ident.sym.to_string());
                }
                Decl::Var(variable_declaration) => {
                    names.extend(collect_var_decl_bound_names(variable_declaration)?);
                }
                _ => {}
            },
            _ => {}
        }
    }

    Ok(names)
}

pub(crate) fn ensure_module_lexical_names_are_unique(module: &Module) -> Result<()> {
    let mut seen = HashSet::new();

    for item in &module.body {
        match item {
            ModuleItem::Stmt(Stmt::Decl(Decl::Fn(function_declaration))) => {
                ensure!(
                    seen.insert(function_declaration.ident.sym.to_string()),
                    "duplicate lexical name `{}`",
                    function_declaration.ident.sym
                );
            }
            ModuleItem::Stmt(Stmt::Decl(Decl::Class(class_declaration))) => {
                ensure!(
                    seen.insert(class_declaration.ident.sym.to_string()),
                    "duplicate lexical name `{}`",
                    class_declaration.ident.sym
                );
            }
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(variable_declaration)))
                if !matches!(variable_declaration.kind, VarDeclKind::Var) =>
            {
                insert_unique_pattern_names(variable_declaration, &mut seen)?;
            }
            ModuleItem::ModuleDecl(ModuleDecl::Import(import)) => {
                for specifier in &import.specifiers {
                    let local_name = match specifier {
                        ImportSpecifier::Named(named) => named.local.sym.to_string(),
                        ImportSpecifier::Default(default) => default.local.sym.to_string(),
                        ImportSpecifier::Namespace(namespace) => namespace.local.sym.to_string(),
                    };
                    ensure!(
                        seen.insert(local_name.clone()),
                        "duplicate lexical name `{local_name}`"
                    );
                }
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => match &export.decl {
                Decl::Fn(function_declaration) => {
                    ensure!(
                        seen.insert(function_declaration.ident.sym.to_string()),
                        "duplicate lexical name `{}`",
                        function_declaration.ident.sym
                    );
                }
                Decl::Class(class_declaration) => {
                    ensure!(
                        seen.insert(class_declaration.ident.sym.to_string()),
                        "duplicate lexical name `{}`",
                        class_declaration.ident.sym
                    );
                }
                Decl::Var(variable_declaration)
                    if !matches!(variable_declaration.kind, VarDeclKind::Var) =>
                {
                    insert_unique_pattern_names(variable_declaration, &mut seen)?;
                }
                _ => {}
            },
            ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultDecl(export_default)) => {
                match &export_default.decl {
                    DefaultDecl::Fn(function_expression) => {
                        if let Some(identifier) = &function_expression.ident {
                            ensure!(
                                seen.insert(identifier.sym.to_string()),
                                "duplicate lexical name `{}`",
                                identifier.sym
                            );
                        }
                    }
                    DefaultDecl::Class(class_expression) => {
                        if let Some(identifier) = &class_expression.ident {
                            ensure!(
                                seen.insert(identifier.sym.to_string()),
                                "duplicate lexical name `{}`",
                                identifier.sym
                            );
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    Ok(())
}

pub(crate) fn collect_pattern_binding_names(pattern: &Pat, names: &mut Vec<String>) -> Result<()> {
    match pattern {
        Pat::Ident(identifier) => {
            let name = identifier.id.sym.to_string();
            if !names.contains(&name) {
                names.push(name);
            }
        }
        Pat::Assign(assign) => collect_pattern_binding_names(&assign.left, names)?,
        Pat::Array(array) => {
            for element in array.elems.iter().flatten() {
                collect_pattern_binding_names(element, names)?;
            }
        }
        Pat::Object(object) => {
            for property in &object.props {
                match property {
                    ObjectPatProp::KeyValue(property) => {
                        collect_pattern_binding_names(&property.value, names)?;
                    }
                    ObjectPatProp::Assign(property) => {
                        let name = property.key.id.sym.to_string();
                        if !names.contains(&name) {
                            names.push(name);
                        }
                    }
                    ObjectPatProp::Rest(rest) => {
                        collect_pattern_binding_names(&rest.arg, names)?;
                    }
                }
            }
        }
        Pat::Rest(rest) => collect_pattern_binding_names(&rest.arg, names)?,
        Pat::Expr(_) | Pat::Invalid(_) => bail!("unsupported binding pattern"),
    }

    Ok(())
}

pub(super) fn insert_unique_pattern_names(
    variable_declaration: &swc_ecma_ast::VarDecl,
    seen: &mut HashSet<String>,
) -> Result<()> {
    for name in collect_var_decl_bound_names(variable_declaration)? {
        ensure!(seen.insert(name.clone()), "duplicate lexical name `{name}`");
    }
    Ok(())
}
