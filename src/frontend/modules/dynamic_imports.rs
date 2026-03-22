use super::*;

pub(super) fn collect_literal_dynamic_import_specifiers(module: &Module) -> Vec<String> {
    let mut specifiers = Vec::new();
    let mut seen = HashSet::new();

    for item in &module.body {
        collect_dynamic_imports_from_module_item(item, &mut specifiers, &mut seen);
    }

    specifiers
}

pub(super) fn collect_literal_dynamic_import_specifiers_in_statements(
    statements: &[Stmt],
) -> Vec<String> {
    let mut specifiers = Vec::new();
    let mut seen = HashSet::new();

    for statement in statements {
        collect_dynamic_imports_from_statement(statement, &mut specifiers, &mut seen);
    }

    specifiers
}

fn collect_dynamic_imports_from_module_item(
    item: &ModuleItem,
    specifiers: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match item {
        ModuleItem::Stmt(statement) => {
            collect_dynamic_imports_from_statement(statement, specifiers, seen)
        }
        ModuleItem::ModuleDecl(declaration) => {
            collect_dynamic_imports_from_module_declaration(declaration, specifiers, seen)
        }
    }
}

fn collect_dynamic_imports_from_module_declaration(
    declaration: &ModuleDecl,
    specifiers: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match declaration {
        ModuleDecl::ExportDecl(export) => {
            collect_dynamic_imports_from_declaration(&export.decl, specifiers, seen);
        }
        ModuleDecl::ExportDefaultDecl(export_default) => match &export_default.decl {
            DefaultDecl::Fn(function) => {
                collect_dynamic_imports_from_function(&function.function, specifiers, seen)
            }
            DefaultDecl::Class(class) => {
                collect_dynamic_imports_from_class(&class.class, specifiers, seen)
            }
            _ => {}
        },
        ModuleDecl::ExportDefaultExpr(export_default) => {
            collect_dynamic_imports_from_expression(&export_default.expr, specifiers, seen);
        }
        ModuleDecl::ExportNamed(_) | ModuleDecl::ExportAll(_) => {}
        _ => {}
    }
}

fn collect_dynamic_imports_from_statement(
    statement: &Stmt,
    specifiers: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match statement {
        Stmt::Block(block) => {
            for statement in &block.stmts {
                collect_dynamic_imports_from_statement(statement, specifiers, seen);
            }
        }
        Stmt::Decl(declaration) => {
            collect_dynamic_imports_from_declaration(declaration, specifiers, seen)
        }
        Stmt::Expr(expression) => {
            collect_dynamic_imports_from_expression(&expression.expr, specifiers, seen)
        }
        Stmt::If(statement) => {
            collect_dynamic_imports_from_expression(&statement.test, specifiers, seen);
            collect_dynamic_imports_from_statement(&statement.cons, specifiers, seen);
            if let Some(alternate) = &statement.alt {
                collect_dynamic_imports_from_statement(alternate, specifiers, seen);
            }
        }
        Stmt::While(statement) => {
            collect_dynamic_imports_from_expression(&statement.test, specifiers, seen);
            collect_dynamic_imports_from_statement(&statement.body, specifiers, seen);
        }
        Stmt::DoWhile(statement) => {
            collect_dynamic_imports_from_statement(&statement.body, specifiers, seen);
            collect_dynamic_imports_from_expression(&statement.test, specifiers, seen);
        }
        Stmt::For(statement) => {
            if let Some(init) = &statement.init {
                match init {
                    VarDeclOrExpr::VarDecl(variable_declaration) => {
                        collect_dynamic_imports_from_variable_declaration(
                            variable_declaration,
                            specifiers,
                            seen,
                        );
                    }
                    VarDeclOrExpr::Expr(expression) => {
                        collect_dynamic_imports_from_expression(expression, specifiers, seen);
                    }
                }
            }
            if let Some(test) = &statement.test {
                collect_dynamic_imports_from_expression(test, specifiers, seen);
            }
            if let Some(update) = &statement.update {
                collect_dynamic_imports_from_expression(update, specifiers, seen);
            }
            collect_dynamic_imports_from_statement(&statement.body, specifiers, seen);
        }
        Stmt::ForIn(statement) => {
            collect_dynamic_imports_from_for_head(&statement.left, specifiers, seen);
            collect_dynamic_imports_from_expression(&statement.right, specifiers, seen);
            collect_dynamic_imports_from_statement(&statement.body, specifiers, seen);
        }
        Stmt::ForOf(statement) => {
            collect_dynamic_imports_from_for_head(&statement.left, specifiers, seen);
            collect_dynamic_imports_from_expression(&statement.right, specifiers, seen);
            collect_dynamic_imports_from_statement(&statement.body, specifiers, seen);
        }
        Stmt::Switch(statement) => {
            collect_dynamic_imports_from_expression(&statement.discriminant, specifiers, seen);
            for case in &statement.cases {
                if let Some(test) = &case.test {
                    collect_dynamic_imports_from_expression(test, specifiers, seen);
                }
                for statement in &case.cons {
                    collect_dynamic_imports_from_statement(statement, specifiers, seen);
                }
            }
        }
        Stmt::Try(statement) => {
            for statement in &statement.block.stmts {
                collect_dynamic_imports_from_statement(statement, specifiers, seen);
            }
            if let Some(handler) = &statement.handler {
                if let Some(pattern) = &handler.param {
                    collect_dynamic_imports_from_pattern(pattern, specifiers, seen);
                }
                for statement in &handler.body.stmts {
                    collect_dynamic_imports_from_statement(statement, specifiers, seen);
                }
            }
            if let Some(finalizer) = &statement.finalizer {
                for statement in &finalizer.stmts {
                    collect_dynamic_imports_from_statement(statement, specifiers, seen);
                }
            }
        }
        Stmt::With(statement) => {
            collect_dynamic_imports_from_expression(&statement.obj, specifiers, seen);
            collect_dynamic_imports_from_statement(&statement.body, specifiers, seen);
        }
        Stmt::Return(statement) => {
            if let Some(argument) = &statement.arg {
                collect_dynamic_imports_from_expression(argument, specifiers, seen);
            }
        }
        Stmt::Throw(statement) => {
            collect_dynamic_imports_from_expression(&statement.arg, specifiers, seen);
        }
        Stmt::Labeled(statement) => {
            collect_dynamic_imports_from_statement(&statement.body, specifiers, seen)
        }
        _ => {}
    }
}

fn collect_dynamic_imports_from_declaration(
    declaration: &Decl,
    specifiers: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match declaration {
        Decl::Fn(function) => {
            collect_dynamic_imports_from_function(&function.function, specifiers, seen)
        }
        Decl::Class(class) => collect_dynamic_imports_from_class(&class.class, specifiers, seen),
        Decl::Var(variable_declaration) => collect_dynamic_imports_from_variable_declaration(
            variable_declaration,
            specifiers,
            seen,
        ),
        _ => {}
    }
}

fn collect_dynamic_imports_from_variable_declaration(
    declaration: &swc_ecma_ast::VarDecl,
    specifiers: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    for declarator in &declaration.decls {
        collect_dynamic_imports_from_pattern(&declarator.name, specifiers, seen);
        if let Some(initializer) = &declarator.init {
            collect_dynamic_imports_from_expression(initializer, specifiers, seen);
        }
    }
}

fn collect_dynamic_imports_from_for_head(
    head: &ForHead,
    specifiers: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match head {
        ForHead::VarDecl(variable_declaration) => {
            collect_dynamic_imports_from_variable_declaration(
                variable_declaration,
                specifiers,
                seen,
            )
        }
        ForHead::Pat(pattern) => collect_dynamic_imports_from_pattern(pattern, specifiers, seen),
        ForHead::UsingDecl(_) => {}
    }
}

fn collect_dynamic_imports_from_pattern(
    pattern: &Pat,
    specifiers: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match pattern {
        Pat::Assign(assign) => {
            collect_dynamic_imports_from_pattern(&assign.left, specifiers, seen);
            collect_dynamic_imports_from_expression(&assign.right, specifiers, seen);
        }
        Pat::Array(array) => {
            for element in array.elems.iter().flatten() {
                collect_dynamic_imports_from_pattern(element, specifiers, seen);
            }
        }
        Pat::Object(object) => {
            for property in &object.props {
                match property {
                    ObjectPatProp::KeyValue(property) => {
                        collect_dynamic_imports_from_pattern(&property.value, specifiers, seen);
                    }
                    ObjectPatProp::Assign(property) => {
                        if let Some(value) = &property.value {
                            collect_dynamic_imports_from_expression(value, specifiers, seen);
                        }
                    }
                    ObjectPatProp::Rest(rest) => {
                        collect_dynamic_imports_from_pattern(&rest.arg, specifiers, seen);
                    }
                }
            }
        }
        Pat::Rest(rest) => collect_dynamic_imports_from_pattern(&rest.arg, specifiers, seen),
        _ => {}
    }
}

fn collect_dynamic_imports_from_function(
    function: &Function,
    specifiers: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    for parameter in &function.params {
        collect_dynamic_imports_from_pattern(&parameter.pat, specifiers, seen);
    }
    if let Some(body) = &function.body {
        for statement in &body.stmts {
            collect_dynamic_imports_from_statement(statement, specifiers, seen);
        }
    }
}

fn collect_dynamic_imports_from_class(
    class: &Class,
    specifiers: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    if let Some(super_class) = &class.super_class {
        collect_dynamic_imports_from_expression(super_class, specifiers, seen);
    }

    for member in &class.body {
        match member {
            ClassMember::Constructor(constructor) => {
                if let Some(body) = &constructor.body {
                    for statement in &body.stmts {
                        collect_dynamic_imports_from_statement(statement, specifiers, seen);
                    }
                }
            }
            ClassMember::Method(method) => {
                collect_dynamic_imports_from_property_name(&method.key, specifiers, seen);
                collect_dynamic_imports_from_function(&method.function, specifiers, seen);
            }
            ClassMember::ClassProp(property) => {
                collect_dynamic_imports_from_property_name(&property.key, specifiers, seen);
                if let Some(value) = &property.value {
                    collect_dynamic_imports_from_expression(value, specifiers, seen);
                }
            }
            ClassMember::PrivateMethod(method) => {
                collect_dynamic_imports_from_function(&method.function, specifiers, seen);
            }
            ClassMember::PrivateProp(property) => {
                if let Some(value) = &property.value {
                    collect_dynamic_imports_from_expression(value, specifiers, seen);
                }
            }
            ClassMember::StaticBlock(block) => {
                for statement in &block.body.stmts {
                    collect_dynamic_imports_from_statement(statement, specifiers, seen);
                }
            }
            _ => {}
        }
    }
}

fn collect_dynamic_imports_from_property_name(
    name: &PropName,
    specifiers: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    if let PropName::Computed(computed) = name {
        collect_dynamic_imports_from_expression(&computed.expr, specifiers, seen);
    }
}

fn collect_dynamic_imports_from_expression(
    expression: &Expr,
    specifiers: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match expression {
        Expr::Call(call) => {
            if matches!(call.callee, Callee::Import(_))
                && call.args.len() == 1
                && call.args[0].spread.is_none()
                && let Expr::Lit(Lit::Str(string)) = &*call.args[0].expr
            {
                let source = string.value.to_string_lossy().to_string();
                if seen.insert(source.clone()) {
                    specifiers.push(source);
                }
            } else if let Callee::Expr(callee) = &call.callee {
                collect_dynamic_imports_from_expression(callee, specifiers, seen);
            }

            for argument in &call.args {
                collect_dynamic_imports_from_expression(&argument.expr, specifiers, seen);
            }
        }
        Expr::New(new_expression) => {
            collect_dynamic_imports_from_expression(&new_expression.callee, specifiers, seen);
            for argument in new_expression.args.iter().flatten() {
                collect_dynamic_imports_from_expression(&argument.expr, specifiers, seen);
            }
        }
        Expr::Await(await_expression) => {
            collect_dynamic_imports_from_expression(&await_expression.arg, specifiers, seen)
        }
        Expr::Yield(yield_expression) => {
            if let Some(argument) = &yield_expression.arg {
                collect_dynamic_imports_from_expression(argument, specifiers, seen);
            }
        }
        Expr::Paren(parenthesized) => {
            collect_dynamic_imports_from_expression(&parenthesized.expr, specifiers, seen)
        }
        Expr::Array(array) => {
            for element in array.elems.iter().flatten() {
                collect_dynamic_imports_from_expression(&element.expr, specifiers, seen);
            }
        }
        Expr::Object(object) => {
            for property in &object.props {
                match property {
                    PropOrSpread::Spread(spread) => {
                        collect_dynamic_imports_from_expression(&spread.expr, specifiers, seen)
                    }
                    PropOrSpread::Prop(property) => match &**property {
                        Prop::Shorthand(_) => {}
                        Prop::KeyValue(property) => {
                            collect_dynamic_imports_from_property_name(
                                &property.key,
                                specifiers,
                                seen,
                            );
                            collect_dynamic_imports_from_expression(
                                &property.value,
                                specifiers,
                                seen,
                            );
                        }
                        Prop::Getter(property) => {
                            collect_dynamic_imports_from_property_name(
                                &property.key,
                                specifiers,
                                seen,
                            );
                            if let Some(body) = &property.body {
                                for statement in &body.stmts {
                                    collect_dynamic_imports_from_statement(
                                        statement, specifiers, seen,
                                    );
                                }
                            }
                        }
                        Prop::Setter(property) => {
                            collect_dynamic_imports_from_property_name(
                                &property.key,
                                specifiers,
                                seen,
                            );
                            collect_dynamic_imports_from_pattern(&property.param, specifiers, seen);
                            if let Some(body) = &property.body {
                                for statement in &body.stmts {
                                    collect_dynamic_imports_from_statement(
                                        statement, specifiers, seen,
                                    );
                                }
                            }
                        }
                        Prop::Method(property) => {
                            collect_dynamic_imports_from_property_name(
                                &property.key,
                                specifiers,
                                seen,
                            );
                            collect_dynamic_imports_from_function(
                                &property.function,
                                specifiers,
                                seen,
                            );
                        }
                        Prop::Assign(property) => collect_dynamic_imports_from_expression(
                            &property.value,
                            specifiers,
                            seen,
                        ),
                    },
                }
            }
        }
        Expr::Member(member) => {
            collect_dynamic_imports_from_expression(&member.obj, specifiers, seen);
            if let MemberProp::Computed(property) = &member.prop {
                collect_dynamic_imports_from_expression(&property.expr, specifiers, seen);
            }
        }
        Expr::Unary(unary) => collect_dynamic_imports_from_expression(&unary.arg, specifiers, seen),
        Expr::Update(update) => {
            collect_dynamic_imports_from_expression(&update.arg, specifiers, seen)
        }
        Expr::Bin(binary) => {
            collect_dynamic_imports_from_expression(&binary.left, specifiers, seen);
            collect_dynamic_imports_from_expression(&binary.right, specifiers, seen);
        }
        Expr::Assign(assignment) => {
            match &assignment.left {
                AssignTarget::Simple(simple) => {
                    if let SimpleAssignTarget::Member(member) = simple {
                        collect_dynamic_imports_from_expression(&member.obj, specifiers, seen);
                        if let MemberProp::Computed(property) = &member.prop {
                            collect_dynamic_imports_from_expression(
                                &property.expr,
                                specifiers,
                                seen,
                            );
                        }
                    }
                }
                AssignTarget::Pat(_) => {}
            }
            collect_dynamic_imports_from_expression(&assignment.right, specifiers, seen);
        }
        Expr::Cond(conditional) => {
            collect_dynamic_imports_from_expression(&conditional.test, specifiers, seen);
            collect_dynamic_imports_from_expression(&conditional.cons, specifiers, seen);
            collect_dynamic_imports_from_expression(&conditional.alt, specifiers, seen);
        }
        Expr::Seq(sequence) => {
            for expression in &sequence.exprs {
                collect_dynamic_imports_from_expression(expression, specifiers, seen);
            }
        }
        Expr::Fn(function) => {
            collect_dynamic_imports_from_function(&function.function, specifiers, seen)
        }
        Expr::Arrow(arrow) => {
            for parameter in &arrow.params {
                collect_dynamic_imports_from_pattern(parameter, specifiers, seen);
            }
            match &*arrow.body {
                BlockStmtOrExpr::BlockStmt(block) => {
                    for statement in &block.stmts {
                        collect_dynamic_imports_from_statement(statement, specifiers, seen);
                    }
                }
                BlockStmtOrExpr::Expr(expression) => {
                    collect_dynamic_imports_from_expression(expression, specifiers, seen);
                }
            }
        }
        Expr::Class(class) => collect_dynamic_imports_from_class(&class.class, specifiers, seen),
        Expr::Tpl(template) => {
            for expression in &template.exprs {
                collect_dynamic_imports_from_expression(expression, specifiers, seen);
            }
        }
        Expr::TaggedTpl(tagged) => {
            collect_dynamic_imports_from_expression(&tagged.tag, specifiers, seen);
            for expression in &tagged.tpl.exprs {
                collect_dynamic_imports_from_expression(expression, specifiers, seen);
            }
        }
        _ => {}
    }
}
