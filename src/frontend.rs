use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use swc_common::{FileName, SourceMap, Span, source_map::SmallPos, sync::Lrc};
use swc_ecma_ast::{
    ArrowExpr, AssignOp, AssignTarget, BinaryOp as SwcBinaryOp, BindingIdent, BlockStmt,
    BlockStmtOrExpr, BreakStmt, Callee, Class, ClassDecl, ClassMember, ClassMethod, Constructor,
    ContinueStmt, Decl, DefaultDecl, ExportDefaultDecl, ExportSpecifier, Expr, ExprStmt, FnDecl,
    FnExpr, ForHead, ForInStmt, ForOfStmt, Function, ImportDecl, ImportSpecifier, LabeledStmt, Lit,
    MemberProp, MetaPropKind, MethodKind, Module, ModuleDecl, ModuleExportName, ModuleItem,
    ObjectLit, ObjectPatProp, ParamOrTsParamProp, Pat, Program as SwcProgram, Prop, PropName,
    PropOrSpread, SimpleAssignTarget, Stmt, SuperProp, SuperPropExpr, SwitchStmt,
    UnaryOp as SwcUnaryOp, UpdateOp as SwcUpdateOp, VarDeclKind, VarDeclOrExpr, WithStmt,
};
use swc_ecma_parser::{Parser, StringInput, Syntax, lexer::Lexer};

use crate::hir::{
    ArrayElement, BinaryOp, CallArgument, Expression, FunctionDeclaration, FunctionKind,
    ObjectEntry, Parameter, Program, Statement, SwitchCase, UnaryOp, UpdateOp,
};

#[path = "frontend/strict_mode.rs"]
mod strict_mode;
#[path = "frontend/syntax.rs"]
mod syntax;

use strict_mode::{
    function_has_use_strict_directive, script_has_use_strict_directive,
    validate_strict_mode_early_errors_in_module_items,
    validate_strict_mode_early_errors_in_statements,
};
use syntax::{
    collect_module_declared_names, collect_pattern_binding_names, collect_var_decl_bound_names,
    ensure_module_lexical_names_are_unique, validate_class_syntax, validate_declaration_syntax,
    validate_expression_syntax, validate_function_syntax, validate_statement_syntax,
};

pub fn parse(source: &str) -> Result<Program> {
    let mut lowered_source = source.to_string();
    let parsed = parse_program_source(source).or_else(|parse_error| {
        let Some(rewritten) = rewrite_script_await_identifiers(source) else {
            return Err(parse_error);
        };
        lowered_source = rewritten;
        parse_program_source(&lowered_source).map_err(|rewrite_error| {
            anyhow::anyhow!(
                "{parse_error:#}\nfailed again after rewriting script-goal `await` identifiers: {rewrite_error:#}"
            )
        })
    })?;

    Lowerer {
        source_text: Some(lowered_source),
        ..Default::default()
    }
    .lower_program(&parsed)
}

pub fn parse_script_goal(source: &str) -> Result<Program> {
    let mut lowered_source = source.to_string();
    let parsed = parse_script_program_source(source).or_else(|parse_error| {
        let Some(rewritten) = rewrite_script_await_identifiers(source) else {
            return Err(parse_error);
        };
        lowered_source = rewritten;
        parse_script_program_source(&lowered_source).map_err(|rewrite_error| {
            anyhow::anyhow!(
                "{parse_error:#}\nfailed again after rewriting script-goal `await` identifiers: {rewrite_error:#}"
            )
        })
    })?;

    Lowerer {
        source_text: Some(lowered_source),
        ..Default::default()
    }
    .lower_program(&parsed)
}

pub fn parse_module_goal(source: &str) -> Result<Program> {
    parse_module_goal_with_path(Path::new("input.js"), source)
}

#[allow(dead_code)]
pub fn validate_script_goal(source: &str) -> Result<()> {
    let normalized = normalize_leading_hashbang_comment(source);
    let source_map: Lrc<SourceMap> = Default::default();
    let file = source_map.new_source_file(
        FileName::Custom("eval.js".into()).into(),
        normalized.into_owned(),
    );
    parse_script(&file).map(|_| ())
}

pub fn parse_module_goal_with_path(path: &Path, source: &str) -> Result<Program> {
    let normalized = normalize_leading_hashbang_comment(source);
    let source_map: Lrc<SourceMap> = Default::default();
    let file = source_map.new_source_file(
        FileName::Real(path.to_path_buf()).into(),
        normalized.into_owned(),
    );

    let parsed = parse_module(&file)?;
    Lowerer {
        source_text: Some(source.to_string()),
        ..Default::default()
    }
    .lower_program(&parsed)
}

pub fn bundle_module_entry(path: &Path) -> Result<Program> {
    ModuleLinker::default().bundle_entry(path)
}

pub fn bundle_script_entry(path: &Path) -> Result<Program> {
    ModuleLinker::default().bundle_script_entry(path)
}

fn parse_program_source(source: &str) -> Result<SwcProgram> {
    let normalized = normalize_leading_hashbang_comment(source);
    let source_map: Lrc<SourceMap> = Default::default();
    let file = source_map.new_source_file(
        FileName::Custom("input.js".into()).into(),
        normalized.into_owned(),
    );

    parse_script(&file).or_else(|script_error| {
        parse_module(&file).map_err(|module_error| {
            anyhow::anyhow!(
                "failed to parse JavaScript source as script: {script_error:#}\nfailed to parse JavaScript source as module: {module_error:#}"
            )
        })
    })
}

fn parse_script_program_source(source: &str) -> Result<SwcProgram> {
    let normalized = normalize_leading_hashbang_comment(source);
    let source_map: Lrc<SourceMap> = Default::default();
    let file = source_map.new_source_file(
        FileName::Custom("input.js".into()).into(),
        normalized.into_owned(),
    );

    parse_script(&file)
}

fn normalize_leading_hashbang_comment(source: &str) -> Cow<'_, str> {
    if let Some(rest) = source.strip_prefix("#!") {
        return Cow::Owned(format!("//{rest}"));
    }

    if let Some(rest) = source.strip_prefix("\u{FEFF}#!") {
        return Cow::Owned(format!("\u{FEFF}//{rest}"));
    }

    Cow::Borrowed(source)
}

fn parse_script(file: &swc_common::SourceFile) -> Result<SwcProgram> {
    let lexer = Lexer::new(
        Syntax::Es(Default::default()),
        Default::default(),
        StringInput::from(file),
        None,
    );
    let mut parser = Parser::new_from(lexer);
    let script = parser
        .parse_script()
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
    if let Some(error) = parser.take_errors().into_iter().next() {
        bail!("{error:?}");
    }
    validate_script_ast(&script, file)?;
    Ok(SwcProgram::Script(script))
}

fn parse_module(file: &swc_common::SourceFile) -> Result<SwcProgram> {
    let lexer = Lexer::new(
        Syntax::Es(Default::default()),
        Default::default(),
        StringInput::from(file),
        None,
    );
    let mut parser = Parser::new_from(lexer);
    let module = parser
        .parse_module()
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
    if let Some(error) = parser.take_errors().into_iter().next() {
        bail!("{error:?}");
    }
    validate_module_ast(&module, file)?;
    Ok(SwcProgram::Module(module))
}

fn validate_script_ast(script: &swc_ecma_ast::Script, file: &swc_common::SourceFile) -> Result<()> {
    for statement in &script.body {
        validate_statement_syntax(statement, file)?;
    }

    validate_strict_mode_early_errors_in_statements(
        &script.body,
        script_has_use_strict_directive(&script.body),
    )?;

    Ok(())
}

fn validate_module_ast(module: &Module, file: &swc_common::SourceFile) -> Result<()> {
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

fn validate_import_attributes(attributes: Option<&ObjectLit>) -> Result<()> {
    let Some(attributes) = attributes else {
        return Ok(());
    };
    let import_with = attributes
        .as_import_with()
        .context("unsupported import attributes syntax")?;
    let mut keys = HashSet::new();
    for item in import_with.values {
        let key = item.key.sym.to_string();
        ensure!(
            keys.insert(key.clone()),
            "duplicate import attribute key `{key}`"
        );
    }
    Ok(())
}

fn parse_module_file(path: &Path) -> Result<(Module, String)> {
    let source_map: Lrc<SourceMap> = Default::default();
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read `{}`", path.display()))?;
    let normalized = normalize_leading_hashbang_comment(&source);
    let file = source_map.new_source_file(
        FileName::Real(path.to_path_buf()).into(),
        normalized.into_owned(),
    );
    let SwcProgram::Module(module) = parse_module(&file)? else {
        unreachable!("parse_module must return a module");
    };
    Ok((module, source))
}

fn parse_script_file(path: &Path) -> Result<(swc_ecma_ast::Script, String)> {
    let source_map: Lrc<SourceMap> = Default::default();
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read `{}`", path.display()))?;
    let parse_once = |source: &str| -> Result<swc_ecma_ast::Script> {
        let normalized = normalize_leading_hashbang_comment(source);
        let file = source_map.new_source_file(
            FileName::Real(path.to_path_buf()).into(),
            normalized.into_owned(),
        );
        let SwcProgram::Script(script) = parse_script(&file)? else {
            unreachable!("parse_script must return a script");
        };
        Ok(script)
    };

    parse_once(&source)
        .map(|script| (script, source.clone()))
        .or_else(|parse_error| {
        let Some(rewritten) = rewrite_script_await_identifiers(&source) else {
            return Err(parse_error);
        };
        parse_once(&rewritten)
            .map(|script| (script, rewritten))
            .map_err(|rewrite_error| {
                anyhow::anyhow!(
                    "{parse_error:#}\nfailed again after rewriting script-goal `await` identifiers: {rewrite_error:#}"
                )
            })
    })
}

#[derive(Default)]
struct ModuleLinker {
    lowerer: Lowerer,
    modules: Vec<LinkedModule>,
    module_indices: HashMap<PathBuf, usize>,
    load_order: Vec<usize>,
}

#[derive(Clone)]
struct LinkedModule {
    path: PathBuf,
    state: ModuleState,
    namespace_name: String,
    init_name: String,
    promise_name: String,
    init_async: bool,
    dependency_params: Vec<ModuleDependencyParam>,
    export_names: Vec<String>,
    export_resolutions: BTreeMap<String, ExportResolution>,
    ambiguous_export_names: HashSet<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModuleState {
    Reserved,
    Lowering,
    Lowered,
}

#[derive(Clone)]
struct ModuleDependencyParam {
    module_index: usize,
    param_name: String,
}

#[derive(Clone)]
enum ImportBinding {
    Namespace {
        module_index: usize,
        namespace_param: String,
    },
    Named {
        module_index: usize,
        namespace_param: String,
        export_name: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ExportResolution {
    Binding {
        module_index: usize,
        binding_name: String,
        local: bool,
    },
    Namespace {
        module_index: usize,
    },
}

impl ModuleLinker {
    fn bundle_entry(&mut self, path: &Path) -> Result<Program> {
        let entry_index = self.load_module(path)?;
        self.load_order = self.compute_static_load_order(entry_index);
        let statements = self.bundle_statements(entry_index)?;
        Ok(self.lowerer.finish_program(statements, true))
    }

    fn bundle_script_entry(&mut self, path: &Path) -> Result<Program> {
        let (script, lowered_source) = parse_script_file(path)?;
        for source in collect_literal_dynamic_import_specifiers_in_statements(&script.body) {
            if let Ok(dependency_path) = resolve_module_specifier(path, &source) {
                self.load_module(&dependency_path)?;
            }
        }

        self.lowerer.source_text = Some(lowered_source);
        self.lowerer.current_module_path = Some(normalize_module_path(path)?);
        self.lowerer.module_index_lookup = self.module_indices.clone();
        let strict = script_has_use_strict_directive(&script.body);
        self.lowerer.strict_modes.push(strict);
        self.lowerer.module_mode = false;

        let mut statements = self.module_registry_statements();
        let scope_bindings = collect_direct_statement_lexical_bindings(&script.body)?;
        self.lowerer.push_binding_scope(scope_bindings);
        let lowered = self
            .lowerer
            .lower_top_level_statements(script.body.iter(), &mut statements);
        self.lowerer.pop_binding_scope();
        lowered?;

        self.lowerer.strict_modes.pop();
        self.lowerer.module_mode = false;
        self.lowerer.source_text = None;
        self.lowerer.current_module_path = None;
        self.lowerer.module_index_lookup.clear();

        Ok(self.lowerer.finish_program(statements, strict))
    }

    fn ensure_module_slot(&mut self, path: &Path) -> Result<usize> {
        let resolved = normalize_module_path(path)?;
        if let Some(index) = self.module_indices.get(&resolved).copied() {
            return Ok(index);
        }

        let module_index = self.modules.len();
        self.module_indices.insert(resolved.clone(), module_index);
        self.modules.push(LinkedModule {
            path: resolved.clone(),
            state: ModuleState::Reserved,
            namespace_name: format!("__ayy_module_namespace_{module_index}"),
            init_name: format!("__ayy_module_init_{module_index}"),
            promise_name: format!("__ayy_module_promise_{module_index}"),
            init_async: false,
            dependency_params: Vec::new(),
            export_names: Vec::new(),
            export_resolutions: BTreeMap::new(),
            ambiguous_export_names: HashSet::new(),
        });

        Ok(module_index)
    }

    fn load_module(&mut self, path: &Path) -> Result<usize> {
        let module_index = self.ensure_module_slot(path)?;
        if self.modules[module_index].state != ModuleState::Reserved {
            return Ok(module_index);
        }

        let resolved = self.modules[module_index].path.clone();
        let (module, source_text) = parse_module_file(&resolved)?;
        self.modules[module_index].state = ModuleState::Lowering;
        self.predeclare_module_export_resolutions(module_index, &module, &resolved)?;
        self.lower_module(module_index, &module, source_text)?;
        self.modules[module_index].state = ModuleState::Lowered;

        Ok(module_index)
    }

    fn lower_module(
        &mut self,
        module_index: usize,
        module: &Module,
        source_text: String,
    ) -> Result<()> {
        let exports_param = "exports".to_string();
        let module_path = self.modules[module_index].path.clone();
        ensure_module_lexical_names_are_unique(module)?;
        let module_declared_names = collect_module_declared_names(module)?;
        let mut dependency_params = Vec::new();
        let mut dependency_param_by_index = HashMap::new();
        let mut import_bindings = HashMap::new();
        let mut export_expressions = BTreeMap::<String, Expression>::new();
        let mut export_resolutions = self.modules[module_index].export_resolutions.clone();
        let mut star_export_expressions = BTreeMap::<String, Expression>::new();
        let mut star_export_resolutions = BTreeMap::<String, ExportResolution>::new();
        let mut ambiguous_star_exports = HashSet::<String>::new();
        let mut pending_self_reexports = Vec::<(String, String)>::new();
        let mut hoisted_statements = Vec::new();
        let mut body_statements = Vec::new();

        for source in collect_literal_dynamic_import_specifiers(module) {
            if let Ok(dependency_path) = resolve_module_specifier(&module_path, &source) {
                self.load_module(&dependency_path)?;
            }
        }

        for item in &module.body {
            let ModuleItem::ModuleDecl(module_declaration) = item else {
                continue;
            };
            match module_declaration {
                ModuleDecl::Import(import) => self.register_import_declaration(
                    &module_path,
                    import,
                    &mut dependency_params,
                    &mut dependency_param_by_index,
                    &mut import_bindings,
                )?,
                ModuleDecl::ExportNamed(export_named) => {
                    if let Some(source) = &export_named.src {
                        self.dependency_param_for_source(
                            &module_path,
                            &source.value.to_string_lossy(),
                            &mut dependency_params,
                            &mut dependency_param_by_index,
                        )?;
                    }
                }
                ModuleDecl::ExportAll(export_all) => {
                    self.dependency_param_for_source(
                        &module_path,
                        &export_all.src.value.to_string_lossy(),
                        &mut dependency_params,
                        &mut dependency_param_by_index,
                    )?;
                }
                _ => {}
            }
        }

        self.lowerer.strict_modes.push(true);
        self.lowerer.module_mode = true;
        self.lowerer.source_text = Some(source_text);
        self.lowerer.current_module_path = Some(module_path.clone());
        self.lowerer.module_index_lookup = self.module_indices.clone();
        let function_start = self.lowerer.functions.len();

        for item in &module.body {
            match item {
                ModuleItem::Stmt(statement) => match statement {
                    Stmt::Decl(Decl::Fn(function_declaration)) => hoisted_statements.extend(
                        self.lowerer
                            .lower_nested_function_declaration(function_declaration)?,
                    ),
                    other => {
                        body_statements.extend(self.lowerer.lower_statement(other, false, false)?)
                    }
                },
                ModuleItem::ModuleDecl(module_declaration) => match module_declaration {
                    ModuleDecl::Import(_) => {}
                    ModuleDecl::ExportDecl(export) => match &export.decl {
                        Decl::Fn(function_declaration) => {
                            hoisted_statements.extend(
                                self.lowerer
                                    .lower_nested_function_declaration(function_declaration)?,
                            );
                            let export_name = function_declaration.ident.sym.to_string();
                            export_expressions.insert(
                                export_name.clone(),
                                Expression::Identifier(export_name.clone()),
                            );
                            export_resolutions.insert(
                                export_name.clone(),
                                ExportResolution::Binding {
                                    module_index,
                                    binding_name: export_name,
                                    local: true,
                                },
                            );
                        }
                        Decl::Var(variable_declaration) => {
                            body_statements.extend(
                                self.lowerer
                                    .lower_variable_declaration(variable_declaration)?,
                            );
                            for name in collect_var_decl_bound_names(variable_declaration)? {
                                export_expressions
                                    .insert(name.clone(), Expression::Identifier(name.clone()));
                                export_resolutions.insert(
                                    name.clone(),
                                    ExportResolution::Binding {
                                        module_index,
                                        binding_name: name,
                                        local: true,
                                    },
                                );
                            }
                        }
                        Decl::Class(class_declaration) => {
                            body_statements
                                .extend(self.lowerer.lower_class_declaration(class_declaration)?);
                            let export_name = class_declaration.ident.sym.to_string();
                            export_expressions.insert(
                                export_name.clone(),
                                Expression::Identifier(export_name.clone()),
                            );
                            export_resolutions.insert(
                                export_name.clone(),
                                ExportResolution::Binding {
                                    module_index,
                                    binding_name: export_name,
                                    local: true,
                                },
                            );
                        }
                        other => bail!("unsupported export declaration: {other:?}"),
                    },
                    ModuleDecl::ExportDefaultDecl(export_default) => {
                        let expression = self.lower_default_export_declaration(
                            export_default,
                            &mut hoisted_statements,
                            &mut body_statements,
                        )?;
                        export_expressions.insert("default".to_string(), expression.clone());
                        export_resolutions.insert(
                            "default".to_string(),
                            ExportResolution::Binding {
                                module_index,
                                binding_name: match expression {
                                    Expression::Identifier(name) => name,
                                    _ => "default".to_string(),
                                },
                                local: true,
                            },
                        );
                    }
                    ModuleDecl::ExportDefaultExpr(export_default) => {
                        let local_name = self.lowerer.fresh_temporary_name("module_default");
                        body_statements.push(Statement::Let {
                            name: local_name.clone(),
                            mutable: false,
                            value: self.lowerer.lower_expression_with_name_hint(
                                &export_default.expr,
                                Some("default"),
                            )?,
                        });
                        export_expressions.insert(
                            "default".to_string(),
                            Expression::Identifier(local_name.clone()),
                        );
                        export_resolutions.insert(
                            "default".to_string(),
                            ExportResolution::Binding {
                                module_index,
                                binding_name: local_name,
                                local: true,
                            },
                        );
                    }
                    ModuleDecl::ExportNamed(export_named) if export_named.src.is_none() => {
                        for specifier in &export_named.specifiers {
                            match specifier {
                                ExportSpecifier::Named(named) => {
                                    let local_name = module_export_name_string(&named.orig)?;
                                    ensure!(
                                        import_bindings.contains_key(&local_name)
                                            || module_declared_names.contains(&local_name),
                                        "unresolvable export `{local_name}`"
                                    );
                                    let export_name = named
                                        .exported
                                        .as_ref()
                                        .map(module_export_name_string)
                                        .transpose()?
                                        .unwrap_or_else(|| local_name.clone());
                                    export_expressions.insert(
                                        export_name.clone(),
                                        Expression::Identifier(local_name.clone()),
                                    );
                                    let resolution = import_bindings
                                        .get(&local_name)
                                        .map(|binding| {
                                            self.export_resolution_for_import_binding(binding)
                                        })
                                        .transpose()?
                                        .unwrap_or_else(|| ExportResolution::Binding {
                                            module_index,
                                            binding_name: local_name,
                                            local: true,
                                        });
                                    export_resolutions.insert(export_name, resolution);
                                }
                                other => bail!("unsupported local export specifier: {other:?}"),
                            }
                        }
                    }
                    ModuleDecl::ExportNamed(export_named) => {
                        let source = export_named
                            .src
                            .as_ref()
                            .context("re-export must have a source")?;
                        let namespace_param = self.dependency_param_for_source(
                            &module_path,
                            &source.value.to_string_lossy(),
                            &mut dependency_params,
                            &mut dependency_param_by_index,
                        )?;
                        let dependency_index = dependency_params
                            .iter()
                            .find(|dependency| dependency.param_name == namespace_param)
                            .map(|dependency| dependency.module_index)
                            .context("re-export dependency must be registered")?;
                        let dependency_finalized = self.load_order.contains(&dependency_index);
                        let self_reexport = dependency_index == module_index;
                        for specifier in &export_named.specifiers {
                            match specifier {
                                ExportSpecifier::Named(named) => {
                                    let imported_name = module_export_name_string(&named.orig)?;
                                    let export_name = named
                                        .exported
                                        .as_ref()
                                        .map(module_export_name_string)
                                        .transpose()?
                                        .unwrap_or_else(|| imported_name.clone());
                                    export_expressions.insert(
                                        export_name.clone(),
                                        Expression::Member {
                                            object: Box::new(Expression::Identifier(
                                                namespace_param.clone(),
                                            )),
                                            property: Box::new(Expression::String(
                                                imported_name.clone(),
                                            )),
                                        },
                                    );
                                    if self_reexport {
                                        pending_self_reexports.push((export_name, imported_name));
                                    } else if !dependency_finalized {
                                        export_resolutions.insert(
                                            export_name,
                                            ExportResolution::Binding {
                                                module_index: dependency_index,
                                                binding_name: imported_name,
                                                local: false,
                                            },
                                        );
                                    } else {
                                        export_resolutions.insert(
                                            export_name,
                                            self.require_export_resolution_for_dependency(
                                                dependency_index,
                                                &imported_name,
                                            )?,
                                        );
                                    }
                                }
                                ExportSpecifier::Namespace(namespace) => {
                                    let export_name = module_export_name_string(&namespace.name)?;
                                    export_expressions.insert(
                                        export_name.clone(),
                                        Expression::Identifier(namespace_param.clone()),
                                    );
                                    export_resolutions.insert(
                                        export_name,
                                        ExportResolution::Namespace {
                                            module_index: dependency_index,
                                        },
                                    );
                                }
                                ExportSpecifier::Default(default) => {
                                    let export_name = default.exported.sym.to_string();
                                    export_expressions.insert(
                                        export_name.clone(),
                                        Expression::Member {
                                            object: Box::new(Expression::Identifier(
                                                namespace_param.clone(),
                                            )),
                                            property: Box::new(Expression::String(
                                                "default".to_string(),
                                            )),
                                        },
                                    );
                                    if self_reexport {
                                        pending_self_reexports
                                            .push((export_name, "default".to_string()));
                                    } else if !dependency_finalized {
                                        export_resolutions.insert(
                                            export_name,
                                            ExportResolution::Binding {
                                                module_index: dependency_index,
                                                binding_name: "default".to_string(),
                                                local: false,
                                            },
                                        );
                                    } else {
                                        export_resolutions.insert(
                                            export_name,
                                            self.require_export_resolution_for_dependency(
                                                dependency_index,
                                                "default",
                                            )?,
                                        );
                                    }
                                }
                            }
                        }
                    }
                    ModuleDecl::ExportAll(export_all) => {
                        let namespace_param = self.dependency_param_for_source(
                            &module_path,
                            &export_all.src.value.to_string_lossy(),
                            &mut dependency_params,
                            &mut dependency_param_by_index,
                        )?;
                        let dependency_index = dependency_params
                            .iter()
                            .find(|dependency| dependency.param_name == namespace_param)
                            .map(|dependency| dependency.module_index)
                            .context("export-all dependency must be registered")?;
                        let dependency_export_resolutions =
                            self.modules[dependency_index].export_resolutions.clone();
                        for (export_name, resolution) in dependency_export_resolutions {
                            if export_name == "default" {
                                continue;
                            }
                            if export_resolutions.contains_key(&export_name)
                                || ambiguous_star_exports.contains(&export_name)
                            {
                                continue;
                            }

                            let expression = Expression::Member {
                                object: Box::new(Expression::Identifier(namespace_param.clone())),
                                property: Box::new(Expression::String(export_name.clone())),
                            };
                            if let Some(previous_resolution) =
                                star_export_resolutions.get(&export_name)
                            {
                                if previous_resolution != &resolution {
                                    star_export_expressions.remove(&export_name);
                                    star_export_resolutions.remove(&export_name);
                                    ambiguous_star_exports.insert(export_name);
                                }
                            } else {
                                star_export_expressions.insert(export_name.clone(), expression);
                                star_export_resolutions.insert(export_name, resolution);
                            }
                        }
                    }
                    other => bail!("unsupported module declaration: {other:?}"),
                },
            }
        }

        for (export_name, expression) in star_export_expressions {
            if !export_expressions.contains_key(&export_name) {
                export_expressions.insert(export_name, expression);
            }
        }
        for (export_name, resolution) in star_export_resolutions {
            if !export_resolutions.contains_key(&export_name) {
                export_resolutions.insert(export_name, resolution);
            }
        }
        for (export_name, imported_name) in pending_self_reexports {
            if let Some(resolution) = export_resolutions.get(&imported_name).cloned() {
                export_resolutions.insert(export_name, resolution);
            } else if let Some(Expression::Identifier(binding_name)) =
                export_expressions.get(&imported_name)
            {
                export_resolutions.insert(
                    export_name,
                    ExportResolution::Binding {
                        module_index,
                        binding_name: binding_name.clone(),
                        local: true,
                    },
                );
            } else if ambiguous_star_exports.contains(&imported_name) {
                bail!(
                    "ambiguous export `{imported_name}` in `{}`",
                    self.modules[module_index].path.display()
                );
            } else {
                bail!(
                    "missing export `{imported_name}` in `{}`",
                    self.modules[module_index].path.display()
                );
            }
        }

        self.lowerer.strict_modes.pop();
        self.lowerer.module_mode = false;
        self.lowerer.source_text = None;
        self.lowerer.current_module_path = None;
        self.lowerer.module_index_lookup.clear();

        self.rewrite_import_bindings_in_statements(&mut hoisted_statements, &import_bindings)?;
        self.rewrite_import_bindings_in_statements(&mut body_statements, &import_bindings)?;
        for function in &mut self.lowerer.functions[function_start..] {
            rewrite_import_bindings_in_function(function, &import_bindings)?;
        }

        let mut init_body = self.build_module_namespace_prelude(&exports_param);
        init_body.extend(self.build_export_getter_statements(
            module_index,
            &exports_param,
            &export_expressions,
            &import_bindings,
        )?);
        init_body.extend(hoisted_statements);
        init_body.extend(body_statements);
        let (init_body, init_async) = asyncify_statements(init_body);

        let mut params = vec![Parameter {
            name: exports_param,
            default: None,
            rest: false,
        }];
        params.extend(dependency_params.iter().map(|dependency| Parameter {
            name: dependency.param_name.clone(),
            default: None,
            rest: false,
        }));

        self.lowerer.functions.push(FunctionDeclaration {
            name: self.modules[module_index].init_name.clone(),
            top_level_binding: None,
            params,
            body: init_body,
            register_global: false,
            kind: if init_async {
                FunctionKind::Async
            } else {
                FunctionKind::Ordinary
            },
            self_binding: None,
            mapped_arguments: false,
            strict: true,
            lexical_this: false,
            length: dependency_params.len() + 1,
        });

        self.modules[module_index].init_async = init_async;
        self.modules[module_index].dependency_params = dependency_params;
        self.modules[module_index].export_names = export_expressions.keys().cloned().collect();
        self.modules[module_index].export_resolutions = export_resolutions;
        self.modules[module_index].ambiguous_export_names = ambiguous_star_exports;

        Ok(())
    }

    fn compute_static_load_order(&self, entry_index: usize) -> Vec<usize> {
        fn visit(
            linker: &ModuleLinker,
            module_index: usize,
            visited: &mut HashSet<usize>,
            order: &mut Vec<usize>,
        ) {
            if !visited.insert(module_index) {
                return;
            }

            for dependency in &linker.modules[module_index].dependency_params {
                if dependency.module_index != module_index {
                    visit(linker, dependency.module_index, visited, order);
                }
            }

            order.push(module_index);
        }

        let mut order = Vec::new();
        visit(self, entry_index, &mut HashSet::new(), &mut order);
        order
    }

    fn predeclare_module_export_resolutions(
        &mut self,
        module_index: usize,
        module: &Module,
        module_path: &Path,
    ) -> Result<()> {
        let module_declared_names = collect_module_declared_names(module)?;
        let mut export_resolutions = BTreeMap::new();
        for item in &module.body {
            let ModuleItem::ModuleDecl(module_declaration) = item else {
                continue;
            };
            match module_declaration {
                ModuleDecl::ExportDecl(export) => match &export.decl {
                    Decl::Fn(function_declaration) => {
                        let export_name = function_declaration.ident.sym.to_string();
                        export_resolutions.insert(
                            export_name.clone(),
                            ExportResolution::Binding {
                                module_index,
                                binding_name: export_name,
                                local: true,
                            },
                        );
                    }
                    Decl::Var(variable_declaration) => {
                        for name in collect_var_decl_bound_names(variable_declaration)? {
                            export_resolutions.insert(
                                name.clone(),
                                ExportResolution::Binding {
                                    module_index,
                                    binding_name: name,
                                    local: true,
                                },
                            );
                        }
                    }
                    Decl::Class(class_declaration) => {
                        let export_name = class_declaration.ident.sym.to_string();
                        export_resolutions.insert(
                            export_name.clone(),
                            ExportResolution::Binding {
                                module_index,
                                binding_name: export_name,
                                local: true,
                            },
                        );
                    }
                    _ => {}
                },
                ModuleDecl::ExportDefaultDecl(export_default) => {
                    let binding_name = match &export_default.decl {
                        DefaultDecl::Class(class_expression) => class_expression
                            .ident
                            .as_ref()
                            .map(|identifier| identifier.sym.to_string())
                            .unwrap_or_else(|| "default".to_string()),
                        DefaultDecl::Fn(function_expression) => function_expression
                            .ident
                            .as_ref()
                            .map(|identifier| identifier.sym.to_string())
                            .unwrap_or_else(|| "default".to_string()),
                        _ => "default".to_string(),
                    };
                    export_resolutions.insert(
                        "default".to_string(),
                        ExportResolution::Binding {
                            module_index,
                            binding_name,
                            local: true,
                        },
                    );
                }
                ModuleDecl::ExportDefaultExpr(_) => {
                    export_resolutions.insert(
                        "default".to_string(),
                        ExportResolution::Binding {
                            module_index,
                            binding_name: "default".to_string(),
                            local: true,
                        },
                    );
                }
                ModuleDecl::ExportNamed(export_named) if export_named.src.is_none() => {
                    for specifier in &export_named.specifiers {
                        let ExportSpecifier::Named(named) = specifier else {
                            continue;
                        };
                        let local_name = module_export_name_string(&named.orig)?;
                        if !module_declared_names.contains(&local_name) {
                            continue;
                        }
                        let export_name = named
                            .exported
                            .as_ref()
                            .map(module_export_name_string)
                            .transpose()?
                            .unwrap_or_else(|| local_name.clone());
                        export_resolutions.insert(
                            export_name,
                            ExportResolution::Binding {
                                module_index,
                                binding_name: local_name,
                                local: true,
                            },
                        );
                    }
                }
                ModuleDecl::ExportNamed(export_named) => {
                    let source = export_named
                        .src
                        .as_ref()
                        .context("re-export must have a source")?;
                    let dependency_path =
                        resolve_module_specifier(module_path, &source.value.to_string_lossy())?;
                    let dependency_index = self.ensure_module_slot(&dependency_path)?;
                    for specifier in &export_named.specifiers {
                        match specifier {
                            ExportSpecifier::Named(named) => {
                                let imported_name = module_export_name_string(&named.orig)?;
                                let export_name = named
                                    .exported
                                    .as_ref()
                                    .map(module_export_name_string)
                                    .transpose()?
                                    .unwrap_or_else(|| imported_name.clone());
                                export_resolutions.insert(
                                    export_name,
                                    ExportResolution::Binding {
                                        module_index: dependency_index,
                                        binding_name: imported_name,
                                        local: false,
                                    },
                                );
                            }
                            ExportSpecifier::Namespace(namespace) => {
                                let export_name = module_export_name_string(&namespace.name)?;
                                export_resolutions.insert(
                                    export_name,
                                    ExportResolution::Namespace {
                                        module_index: dependency_index,
                                    },
                                );
                            }
                            ExportSpecifier::Default(default) => {
                                export_resolutions.insert(
                                    default.exported.sym.to_string(),
                                    ExportResolution::Binding {
                                        module_index: dependency_index,
                                        binding_name: "default".to_string(),
                                        local: false,
                                    },
                                );
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        self.modules[module_index].export_resolutions = export_resolutions;
        Ok(())
    }

    fn require_export_resolution_for_dependency(
        &self,
        module_index: usize,
        export_name: &str,
    ) -> Result<ExportResolution> {
        self.require_export_resolution_for_dependency_with_visited(
            module_index,
            export_name,
            &mut HashSet::new(),
        )
    }

    fn require_export_resolution_for_dependency_with_visited(
        &self,
        module_index: usize,
        export_name: &str,
        visited: &mut HashSet<(usize, String)>,
    ) -> Result<ExportResolution> {
        if !visited.insert((module_index, export_name.to_string())) {
            bail!(
                "circular indirect export `{export_name}` in `{}`",
                self.modules[module_index].path.display()
            );
        }

        if let Some(resolution) = self.modules[module_index]
            .export_resolutions
            .get(export_name)
            .cloned()
        {
            return match resolution {
                ExportResolution::Binding {
                    module_index,
                    binding_name,
                    local: true,
                } => Ok(ExportResolution::Binding {
                    module_index,
                    binding_name,
                    local: true,
                }),
                ExportResolution::Binding {
                    module_index,
                    binding_name,
                    local: false,
                } => self.require_export_resolution_for_dependency_with_visited(
                    module_index,
                    &binding_name,
                    visited,
                ),
                ExportResolution::Namespace { module_index } => {
                    Ok(ExportResolution::Namespace { module_index })
                }
            };
        }

        if self.modules[module_index]
            .ambiguous_export_names
            .contains(export_name)
        {
            bail!(
                "ambiguous export `{export_name}` in `{}`",
                self.modules[module_index].path.display()
            );
        }

        bail!(
            "missing export `{export_name}` in `{}`",
            self.modules[module_index].path.display()
        );
    }

    fn export_resolution_for_import_binding(
        &self,
        binding: &ImportBinding,
    ) -> Result<ExportResolution> {
        match binding {
            ImportBinding::Namespace { module_index, .. } => Ok(ExportResolution::Namespace {
                module_index: *module_index,
            }),
            ImportBinding::Named {
                module_index,
                export_name,
                ..
            } => self.require_export_resolution_for_dependency(*module_index, export_name),
        }
    }

    fn register_import_declaration(
        &mut self,
        module_path: &Path,
        import: &ImportDecl,
        dependency_params: &mut Vec<ModuleDependencyParam>,
        dependency_param_by_index: &mut HashMap<usize, String>,
        import_bindings: &mut HashMap<String, ImportBinding>,
    ) -> Result<()> {
        ensure!(!import.type_only, "type-only imports are not supported yet");
        validate_import_attributes(import.with.as_deref())?;
        let namespace_param = self.dependency_param_for_source(
            module_path,
            &import.src.value.to_string_lossy(),
            dependency_params,
            dependency_param_by_index,
        )?;
        let dependency_index = dependency_params
            .iter()
            .find(|dependency| dependency.param_name == namespace_param)
            .map(|dependency| dependency.module_index)
            .context("import dependency must be registered")?;

        for specifier in &import.specifiers {
            match specifier {
                ImportSpecifier::Named(named) => {
                    ensure!(
                        !named.is_type_only,
                        "type-only named imports are not supported yet"
                    );
                    let export_name = named
                        .imported
                        .as_ref()
                        .map(module_export_name_string)
                        .transpose()?
                        .unwrap_or_else(|| named.local.sym.to_string());
                    self.require_export_resolution_for_dependency(dependency_index, &export_name)?;
                    import_bindings.insert(
                        named.local.sym.to_string(),
                        ImportBinding::Named {
                            module_index: dependency_index,
                            namespace_param: namespace_param.clone(),
                            export_name,
                        },
                    );
                }
                ImportSpecifier::Default(default) => {
                    self.require_export_resolution_for_dependency(dependency_index, "default")?;
                    import_bindings.insert(
                        default.local.sym.to_string(),
                        ImportBinding::Named {
                            module_index: dependency_index,
                            namespace_param: namespace_param.clone(),
                            export_name: "default".to_string(),
                        },
                    );
                }
                ImportSpecifier::Namespace(namespace) => {
                    import_bindings.insert(
                        namespace.local.sym.to_string(),
                        ImportBinding::Namespace {
                            module_index: dependency_index,
                            namespace_param: namespace_param.clone(),
                        },
                    );
                }
            }
        }

        Ok(())
    }

    fn dependency_param_for_source(
        &mut self,
        module_path: &Path,
        source: &str,
        dependency_params: &mut Vec<ModuleDependencyParam>,
        dependency_param_by_index: &mut HashMap<usize, String>,
    ) -> Result<String> {
        let dependency_path = resolve_module_specifier(module_path, source)?;
        let dependency_index = self.load_module(&dependency_path)?;
        if let Some(existing) = dependency_param_by_index.get(&dependency_index) {
            return Ok(existing.clone());
        }

        let param_name = self.lowerer.fresh_temporary_name("module_dep");
        dependency_param_by_index.insert(dependency_index, param_name.clone());
        dependency_params.push(ModuleDependencyParam {
            module_index: dependency_index,
            param_name: param_name.clone(),
        });
        Ok(param_name)
    }

    fn lower_default_export_declaration(
        &mut self,
        export_default: &ExportDefaultDecl,
        hoisted_statements: &mut Vec<Statement>,
        body_statements: &mut Vec<Statement>,
    ) -> Result<Expression> {
        match &export_default.decl {
            DefaultDecl::Fn(function_expression) => {
                if let Some(identifier) = &function_expression.ident {
                    let generated_name = self
                        .lowerer
                        .lower_named_default_function_expression(function_expression)?;
                    hoisted_statements.push(Statement::Let {
                        name: identifier.sym.to_string(),
                        mutable: true,
                        value: Expression::Identifier(generated_name),
                    });
                    Ok(Expression::Identifier(identifier.sym.to_string()))
                } else {
                    let local_name = self.lowerer.fresh_temporary_name("module_default");
                    hoisted_statements.push(Statement::Let {
                        name: local_name.clone(),
                        mutable: true,
                        value: self
                            .lowerer
                            .lower_function_expression(function_expression, Some("default"))?,
                    });
                    Ok(Expression::Identifier(local_name))
                }
            }
            DefaultDecl::Class(class_expression) => {
                let local_name = class_expression
                    .ident
                    .as_ref()
                    .map(|identifier| identifier.sym.to_string())
                    .unwrap_or_else(|| "default".to_string());
                body_statements.extend(
                    self.lowerer
                        .lower_class_definition(&class_expression.class, local_name.clone())?,
                );
                Ok(Expression::Identifier(local_name))
            }
            other => bail!("unsupported default export declaration: {other:?}"),
        }
    }

    fn build_module_namespace_prelude(&self, exports_param: &str) -> Vec<Statement> {
        vec![
            define_property_statement(
                Expression::Identifier(exports_param.to_string()),
                Expression::Member {
                    object: Box::new(Expression::Identifier("Symbol".to_string())),
                    property: Box::new(Expression::String("toStringTag".to_string())),
                },
                Expression::Object(vec![
                    ObjectEntry::Data {
                        key: Expression::String("value".to_string()),
                        value: Expression::String("Module".to_string()),
                    },
                    ObjectEntry::Data {
                        key: Expression::String("writable".to_string()),
                        value: Expression::Bool(false),
                    },
                    ObjectEntry::Data {
                        key: Expression::String("enumerable".to_string()),
                        value: Expression::Bool(false),
                    },
                    ObjectEntry::Data {
                        key: Expression::String("configurable".to_string()),
                        value: Expression::Bool(false),
                    },
                ]),
            ),
            define_property_statement(
                Expression::Identifier(exports_param.to_string()),
                Expression::String("__ayy$module$namespace".to_string()),
                data_property_descriptor(Expression::Bool(true), false, false, false),
            ),
        ]
    }

    fn build_export_getter_statements(
        &mut self,
        module_index: usize,
        exports_param: &str,
        export_expressions: &BTreeMap<String, Expression>,
        import_bindings: &HashMap<String, ImportBinding>,
    ) -> Result<Vec<Statement>> {
        let mut statements = Vec::new();

        for (export_name, expression) in export_expressions {
            let getter_name = format!(
                "__ayy_module_export_getter_{}_{}",
                module_index,
                self.lowerer.fresh_temporary_name("getter")
            );
            let mut getter_function = FunctionDeclaration {
                name: getter_name.clone(),
                top_level_binding: None,
                params: Vec::new(),
                body: vec![Statement::Return(expression.clone())],
                register_global: false,
                kind: FunctionKind::Ordinary,
                self_binding: None,
                mapped_arguments: false,
                strict: true,
                lexical_this: false,
                length: 0,
            };
            rewrite_import_bindings_in_function(&mut getter_function, import_bindings)?;
            self.lowerer.functions.push(getter_function);

            statements.push(define_property_statement(
                Expression::Identifier(exports_param.to_string()),
                Expression::String(export_name.clone()),
                Expression::Object(vec![
                    ObjectEntry::Data {
                        key: Expression::String("get".to_string()),
                        value: Expression::Identifier(getter_name),
                    },
                    ObjectEntry::Data {
                        key: Expression::String("enumerable".to_string()),
                        value: Expression::Bool(true),
                    },
                    ObjectEntry::Data {
                        key: Expression::String("configurable".to_string()),
                        value: Expression::Bool(false),
                    },
                ]),
            ));
        }

        Ok(statements)
    }

    fn module_registry_statements(&self) -> Vec<Statement> {
        let mut statements = Vec::new();

        for module in &self.modules {
            statements.push(Statement::Let {
                name: module.namespace_name.clone(),
                mutable: false,
                value: Expression::Call {
                    callee: Box::new(Expression::Member {
                        object: Box::new(Expression::Identifier("Object".to_string())),
                        property: Box::new(Expression::String("create".to_string())),
                    }),
                    arguments: vec![CallArgument::Expression(Expression::Null)],
                },
            });
        }

        statements
    }

    fn module_init_call_arguments(&self, module_index: usize) -> Vec<CallArgument> {
        let module = &self.modules[module_index];
        let mut arguments = vec![CallArgument::Expression(Expression::Identifier(
            module.namespace_name.clone(),
        ))];
        for dependency in &module.dependency_params {
            arguments.push(CallArgument::Expression(Expression::Identifier(
                self.modules[dependency.module_index].namespace_name.clone(),
            )));
        }
        arguments
    }

    fn bundle_statements(&self, entry_index: usize) -> Result<Vec<Statement>> {
        let mut statements = self.module_registry_statements();

        for &module_index in &self.load_order {
            let module = &self.modules[module_index];
            statements.push(Statement::Let {
                name: module.promise_name.clone(),
                mutable: false,
                value: Expression::Call {
                    callee: Box::new(Expression::Identifier(module.init_name.clone())),
                    arguments: self.module_init_call_arguments(module_index),
                },
            });
        }

        statements.push(Statement::Expression(Expression::Await(Box::new(
            Expression::Identifier(self.modules[entry_index].promise_name.clone()),
        ))));

        Ok(statements)
    }

    fn rewrite_import_bindings_in_statements(
        &self,
        statements: &mut [Statement],
        import_bindings: &HashMap<String, ImportBinding>,
    ) -> Result<()> {
        let mut rewriter = ImportBindingRewriter::new(import_bindings);
        rewriter.rewrite_statement_list(statements)
    }
}

fn normalize_module_path(path: &Path) -> Result<PathBuf> {
    path.canonicalize()
        .with_context(|| format!("failed to resolve module path `{}`", path.display()))
}

fn resolve_module_specifier(module_path: &Path, source: &str) -> Result<PathBuf> {
    ensure!(
        source.starts_with("./") || source.starts_with("../") || source.starts_with('/'),
        "unsupported module specifier `{source}`"
    );
    let candidate = if source.starts_with('/') {
        PathBuf::from(source)
    } else {
        module_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(source)
    };
    normalize_module_path(&candidate)
}

fn collect_literal_dynamic_import_specifiers(module: &Module) -> Vec<String> {
    let mut specifiers = Vec::new();
    let mut seen = HashSet::new();

    for item in &module.body {
        collect_dynamic_imports_from_module_item(item, &mut specifiers, &mut seen);
    }

    specifiers
}

fn collect_literal_dynamic_import_specifiers_in_statements(statements: &[Stmt]) -> Vec<String> {
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

fn module_export_name_string(name: &ModuleExportName) -> Result<String> {
    match name {
        ModuleExportName::Ident(identifier) => Ok(identifier.sym.to_string()),
        ModuleExportName::Str(string) => string
            .value
            .as_str()
            .map(str::to_string)
            .context("malformed module export name"),
    }
}

fn define_property_statement(
    target: Expression,
    property: Expression,
    descriptor: Expression,
) -> Statement {
    Statement::Expression(Expression::Call {
        callee: Box::new(Expression::Member {
            object: Box::new(Expression::Identifier("Object".to_string())),
            property: Box::new(Expression::String("defineProperty".to_string())),
        }),
        arguments: vec![
            CallArgument::Expression(target),
            CallArgument::Expression(property),
            CallArgument::Expression(descriptor),
        ],
    })
}

fn data_property_descriptor(
    value: Expression,
    writable: bool,
    enumerable: bool,
    configurable: bool,
) -> Expression {
    Expression::Object(vec![
        ObjectEntry::Data {
            key: Expression::String("value".to_string()),
            value,
        },
        ObjectEntry::Data {
            key: Expression::String("writable".to_string()),
            value: Expression::Bool(writable),
        },
        ObjectEntry::Data {
            key: Expression::String("enumerable".to_string()),
            value: Expression::Bool(enumerable),
        },
        ObjectEntry::Data {
            key: Expression::String("configurable".to_string()),
            value: Expression::Bool(configurable),
        },
    ])
}

fn getter_property_descriptor(
    getter: Expression,
    enumerable: bool,
    configurable: bool,
) -> Expression {
    Expression::Object(vec![
        ObjectEntry::Data {
            key: Expression::String("get".to_string()),
            value: getter,
        },
        ObjectEntry::Data {
            key: Expression::String("enumerable".to_string()),
            value: Expression::Bool(enumerable),
        },
        ObjectEntry::Data {
            key: Expression::String("configurable".to_string()),
            value: Expression::Bool(configurable),
        },
    ])
}

fn setter_property_descriptor(
    setter: Expression,
    enumerable: bool,
    configurable: bool,
) -> Expression {
    Expression::Object(vec![
        ObjectEntry::Data {
            key: Expression::String("set".to_string()),
            value: setter,
        },
        ObjectEntry::Data {
            key: Expression::String("enumerable".to_string()),
            value: Expression::Bool(enumerable),
        },
        ObjectEntry::Data {
            key: Expression::String("configurable".to_string()),
            value: Expression::Bool(configurable),
        },
    ])
}

struct ImportBindingRewriter<'a> {
    import_bindings: &'a HashMap<String, ImportBinding>,
    scopes: Vec<HashSet<String>>,
}

#[derive(Default)]
struct BindingScope {
    names: Vec<String>,
    renames: HashMap<String, String>,
}

impl<'a> ImportBindingRewriter<'a> {
    fn new(import_bindings: &'a HashMap<String, ImportBinding>) -> Self {
        Self {
            import_bindings,
            scopes: Vec::new(),
        }
    }

    fn rewrite_statement_list(&mut self, statements: &mut [Statement]) -> Result<()> {
        self.scopes.push(
            collect_statement_bindings(statements.iter())
                .into_iter()
                .collect(),
        );
        for statement in statements {
            self.rewrite_statement(statement)?;
        }
        self.scopes.pop();
        Ok(())
    }

    fn rewrite_statement(&mut self, statement: &mut Statement) -> Result<()> {
        match statement {
            Statement::Block { body } | Statement::Labeled { body, .. } => {
                self.rewrite_statement_list(body)
            }
            Statement::Var { value, .. }
            | Statement::Let { value, .. }
            | Statement::Expression(value)
            | Statement::Throw(value)
            | Statement::Return(value)
            | Statement::Yield { value }
            | Statement::YieldDelegate { value } => self.rewrite_expression(value),
            Statement::Assign { name, value } => {
                if !self.is_shadowed(name)
                    && let Some(binding) = self.import_bindings.get(name)
                {
                    return match binding {
                        ImportBinding::Named {
                            namespace_param,
                            export_name,
                            ..
                        } => {
                            self.rewrite_expression(value)?;
                            let value = value.clone();
                            *statement = Statement::AssignMember {
                                object: Expression::Identifier(namespace_param.clone()),
                                property: Expression::String(export_name.clone()),
                                value,
                            };
                            Ok(())
                        }
                        ImportBinding::Namespace { .. } => {
                            bail!("assignment to namespace import `{name}` is not supported yet")
                        }
                    };
                }
                self.rewrite_expression(value)
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                self.rewrite_expression(object)?;
                self.rewrite_expression(property)?;
                self.rewrite_expression(value)
            }
            Statement::Print { values } => {
                for value in values {
                    self.rewrite_expression(value)?;
                }
                Ok(())
            }
            Statement::With { object, body } => {
                self.rewrite_expression(object)?;
                self.rewrite_statement_list(body)
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.rewrite_expression(condition)?;
                self.rewrite_statement_list(then_branch)?;
                self.rewrite_statement_list(else_branch)
            }
            Statement::Try {
                body,
                catch_binding,
                catch_setup,
                catch_body,
            } => {
                self.rewrite_statement_list(body)?;
                let mut catch_scope: HashSet<String> =
                    collect_statement_bindings(catch_setup.iter().chain(catch_body.iter()))
                        .into_iter()
                        .collect();
                if let Some(catch_binding) = catch_binding {
                    catch_scope.insert(catch_binding.clone());
                }
                self.scopes.push(catch_scope);
                for statement in catch_setup {
                    self.rewrite_statement(statement)?;
                }
                for statement in catch_body {
                    self.rewrite_statement(statement)?;
                }
                self.scopes.pop();
                Ok(())
            }
            Statement::Switch {
                bindings,
                discriminant,
                cases,
                ..
            } => {
                self.rewrite_expression(discriminant)?;
                self.scopes.push(bindings.iter().cloned().collect());
                for case in cases {
                    if let Some(test) = &mut case.test {
                        self.rewrite_expression(test)?;
                    }
                    self.rewrite_statement_list(&mut case.body)?;
                }
                self.scopes.pop();
                Ok(())
            }
            Statement::For {
                init,
                per_iteration_bindings,
                condition,
                update,
                break_hook,
                body,
                ..
            } => {
                let mut loop_bindings: HashSet<String> = collect_statement_bindings(init.iter())
                    .into_iter()
                    .collect();
                loop_bindings.extend(per_iteration_bindings.iter().cloned());
                self.scopes.push(loop_bindings);
                for statement in init {
                    self.rewrite_statement(statement)?;
                }
                if let Some(condition) = condition {
                    self.rewrite_expression(condition)?;
                }
                if let Some(update) = update {
                    self.rewrite_expression(update)?;
                }
                if let Some(break_hook) = break_hook {
                    self.rewrite_expression(break_hook)?;
                }
                self.rewrite_statement_list(body)?;
                self.scopes.pop();
                Ok(())
            }
            Statement::While {
                condition,
                break_hook,
                body,
                ..
            }
            | Statement::DoWhile {
                condition,
                break_hook,
                body,
                ..
            } => {
                self.rewrite_expression(condition)?;
                if let Some(break_hook) = break_hook {
                    self.rewrite_expression(break_hook)?;
                }
                self.rewrite_statement_list(body)
            }
            Statement::Break { .. } | Statement::Continue { .. } => Ok(()),
        }
    }

    fn rewrite_expression(&mut self, expression: &mut Expression) -> Result<()> {
        match expression {
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                            self.rewrite_expression(expression)?
                        }
                    }
                }
                Ok(())
            }
            Expression::Sequence(elements) => {
                for element in elements {
                    self.rewrite_expression(element)?;
                }
                Ok(())
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        ObjectEntry::Data { key, value } => {
                            self.rewrite_expression(key)?;
                            self.rewrite_expression(value)?;
                        }
                        ObjectEntry::Getter { key, getter }
                        | ObjectEntry::Setter {
                            key,
                            setter: getter,
                        } => {
                            self.rewrite_expression(key)?;
                            self.rewrite_expression(getter)?;
                        }
                        ObjectEntry::Spread(expression) => {
                            self.rewrite_expression(expression)?;
                        }
                    }
                }
                Ok(())
            }
            Expression::Identifier(name) => {
                if !self.is_shadowed(name)
                    && let Some(binding) = self.import_bindings.get(name)
                {
                    *expression = import_binding_expression(binding);
                }
                Ok(())
            }
            Expression::Member { object, property } => {
                self.rewrite_expression(object)?;
                self.rewrite_expression(property)
            }
            Expression::SuperMember { property } => self.rewrite_expression(property),
            Expression::Assign { name, value } => {
                if !self.is_shadowed(name)
                    && let Some(binding) = self.import_bindings.get(name)
                {
                    return match binding {
                        ImportBinding::Named {
                            namespace_param,
                            export_name,
                            ..
                        } => {
                            self.rewrite_expression(value)?;
                            let value = value.as_ref().clone();
                            *expression = Expression::AssignMember {
                                object: Box::new(Expression::Identifier(namespace_param.clone())),
                                property: Box::new(Expression::String(export_name.clone())),
                                value: Box::new(value),
                            };
                            Ok(())
                        }
                        ImportBinding::Namespace { .. } => {
                            bail!("assignment to namespace import `{name}` is not supported yet")
                        }
                    };
                }
                self.rewrite_expression(value)
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.rewrite_expression(object)?;
                self.rewrite_expression(property)?;
                self.rewrite_expression(value)
            }
            Expression::AssignSuperMember { property, value } => {
                self.rewrite_expression(property)?;
                self.rewrite_expression(value)
            }
            Expression::EnumerateKeys(expression)
            | Expression::Await(expression)
            | Expression::GetIterator(expression)
            | Expression::IteratorClose(expression)
            | Expression::Unary { expression, .. } => self.rewrite_expression(expression),
            Expression::Binary { left, right, .. } => {
                self.rewrite_expression(left)?;
                self.rewrite_expression(right)
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.rewrite_expression(condition)?;
                self.rewrite_expression(then_expression)?;
                self.rewrite_expression(else_expression)
            }
            Expression::Call { callee, arguments }
            | Expression::SuperCall { callee, arguments }
            | Expression::New { callee, arguments } => {
                self.rewrite_expression(callee)?;
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(argument) | CallArgument::Spread(argument) => {
                            self.rewrite_expression(argument)?
                        }
                    }
                }
                Ok(())
            }
            Expression::Update { name, .. } => {
                ensure!(
                    self.is_shadowed(name) || !self.import_bindings.contains_key(name),
                    "update of imported binding `{name}` is not supported yet"
                );
                Ok(())
            }
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::NewTarget
            | Expression::This
            | Expression::Sent => Ok(()),
        }
    }

    fn rewrite_function(&mut self, function: &mut FunctionDeclaration) -> Result<()> {
        let mut function_scope: HashSet<String> = function
            .params
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect();
        if let Some(self_binding) = &function.self_binding {
            function_scope.insert(self_binding.clone());
        }
        function_scope.insert("arguments".to_string());
        self.scopes.push(function_scope);
        self.rewrite_statement_list(&mut function.body)?;
        self.scopes.pop();
        Ok(())
    }

    fn is_shadowed(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|scope| scope.contains(name))
    }
}

fn rewrite_import_bindings_in_function(
    function: &mut FunctionDeclaration,
    import_bindings: &HashMap<String, ImportBinding>,
) -> Result<()> {
    ImportBindingRewriter::new(import_bindings).rewrite_function(function)
}

fn import_binding_expression(binding: &ImportBinding) -> Expression {
    match binding {
        ImportBinding::Namespace {
            namespace_param, ..
        } => Expression::Identifier(namespace_param.clone()),
        ImportBinding::Named {
            namespace_param,
            export_name,
            ..
        } => Expression::Member {
            object: Box::new(Expression::Identifier(namespace_param.clone())),
            property: Box::new(Expression::String(export_name.clone())),
        },
    }
}

fn collect_statement_bindings<'a>(statements: impl Iterator<Item = &'a Statement>) -> Vec<String> {
    let mut bindings = Vec::new();
    let mut seen = HashSet::new();
    for statement in statements {
        match statement {
            Statement::Var { name, .. } | Statement::Let { name, .. } => {
                if seen.insert(name.clone()) {
                    bindings.push(name.clone());
                }
            }
            _ => {}
        }
    }
    bindings
}

fn rewrite_script_await_identifiers(source: &str) -> Option<String> {
    #[derive(Clone, Copy)]
    enum State {
        Code,
        SingleQuoted,
        DoubleQuoted,
        Template,
        LineComment,
        BlockComment,
    }

    fn is_ident_start(character: char) -> bool {
        character == '_' || character == '$' || character.is_ascii_alphabetic()
    }

    fn is_ident_continue(character: char) -> bool {
        is_ident_start(character) || character.is_ascii_digit()
    }

    fn starts_unicode_escape(characters: &[char], index: usize) -> bool {
        matches!(
            characters.get(index..index + 6),
            Some(['\\', 'u', a, b, c, d])
                if a.is_ascii_hexdigit()
                    && b.is_ascii_hexdigit()
                    && c.is_ascii_hexdigit()
                    && d.is_ascii_hexdigit()
        )
    }

    fn decode_unicode_escape(characters: &[char], index: usize) -> Option<(usize, char)> {
        let digits = characters.get(index + 2..index + 6)?;
        let hex = digits.iter().collect::<String>();
        let value = u32::from_str_radix(&hex, 16).ok()?;
        Some((index + 6, char::from_u32(value)?))
    }

    let characters = source.chars().collect::<Vec<_>>();
    let mut rewritten = String::with_capacity(source.len());
    let mut state = State::Code;
    let mut index = 0;
    let mut changed = false;

    while index < characters.len() {
        let character = characters[index];
        let next = characters.get(index + 1).copied();

        match state {
            State::Code => {
                if character == '\'' {
                    state = State::SingleQuoted;
                    rewritten.push(character);
                    index += 1;
                    continue;
                }
                if character == '"' {
                    state = State::DoubleQuoted;
                    rewritten.push(character);
                    index += 1;
                    continue;
                }
                if character == '`' {
                    state = State::Template;
                    rewritten.push(character);
                    index += 1;
                    continue;
                }
                if character == '/' && next == Some('/') {
                    state = State::LineComment;
                    rewritten.push('/');
                    rewritten.push('/');
                    index += 2;
                    continue;
                }
                if character == '/' && next == Some('*') {
                    state = State::BlockComment;
                    rewritten.push('/');
                    rewritten.push('*');
                    index += 2;
                    continue;
                }
                if is_ident_start(character) || starts_unicode_escape(&characters, index) {
                    let mut word = String::new();
                    while index < characters.len() {
                        if is_ident_continue(characters[index]) {
                            word.push(characters[index]);
                            index += 1;
                        } else if starts_unicode_escape(&characters, index) {
                            let Some((next_index, decoded)) =
                                decode_unicode_escape(&characters, index)
                            else {
                                break;
                            };
                            word.push(decoded);
                            index = next_index;
                        } else {
                            break;
                        }
                    }

                    if word == "await" {
                        rewritten.push_str("__ayy_await_ident");
                        changed = true;
                    } else {
                        rewritten.push_str(&word);
                    }
                    continue;
                }

                rewritten.push(character);
                index += 1;
            }
            State::SingleQuoted => {
                rewritten.push(character);
                index += 1;
                if character == '\\' && index < characters.len() {
                    rewritten.push(characters[index]);
                    index += 1;
                } else if character == '\'' {
                    state = State::Code;
                }
            }
            State::DoubleQuoted => {
                rewritten.push(character);
                index += 1;
                if character == '\\' && index < characters.len() {
                    rewritten.push(characters[index]);
                    index += 1;
                } else if character == '"' {
                    state = State::Code;
                }
            }
            State::Template => {
                rewritten.push(character);
                index += 1;
                if character == '\\' && index < characters.len() {
                    rewritten.push(characters[index]);
                    index += 1;
                } else if character == '`' {
                    state = State::Code;
                }
            }
            State::LineComment => {
                rewritten.push(character);
                index += 1;
                if character == '\n' || character == '\r' {
                    state = State::Code;
                }
            }
            State::BlockComment => {
                rewritten.push(character);
                index += 1;
                if character == '*' && next == Some('/') {
                    rewritten.push('/');
                    index += 1;
                    state = State::Code;
                }
            }
        }
    }

    changed.then_some(rewritten)
}

#[derive(Default)]
struct Lowerer {
    source_text: Option<String>,
    functions: Vec<FunctionDeclaration>,
    next_function_expression_id: usize,
    next_temporary_id: usize,
    binding_scopes: Vec<BindingScope>,
    active_binding_counts: HashMap<String, usize>,
    private_name_scopes: Vec<HashMap<String, String>>,
    constructor_super_stack: Vec<Option<String>>,
    strict_modes: Vec<bool>,
    module_mode: bool,
    current_module_path: Option<PathBuf>,
    module_index_lookup: HashMap<PathBuf, usize>,
}

impl Lowerer {
    fn source_span_snippet(&self, span: Span) -> Option<&str> {
        let source = self.source_text.as_deref()?;
        if span.lo.is_dummy() || span.hi.is_dummy() {
            return None;
        }
        let start = span.lo.0.saturating_sub(1) as usize;
        let end = span.hi.0.saturating_sub(1) as usize;
        source.get(start..end)
    }

    fn pure_array_pattern_elision_count(&self, array: &swc_ecma_ast::ArrayPat) -> usize {
        if !array.elems.is_empty() {
            return 0;
        }
        let Some(snippet) = self.source_span_snippet(array.span) else {
            return 0;
        };
        let Some(inner) = snippet
            .strip_prefix('[')
            .and_then(|text| text.strip_suffix(']'))
        else {
            return 0;
        };
        if inner
            .chars()
            .all(|character| character.is_whitespace() || character == ',')
        {
            inner.chars().filter(|&character| character == ',').count()
        } else {
            0
        }
    }

    fn lower_program(&mut self, program: &SwcProgram) -> Result<Program> {
        let mut statements = Vec::new();
        let strict_mode = match program {
            SwcProgram::Script(script) => script_has_use_strict_directive(&script.body),
            SwcProgram::Module(_) => true,
        };
        self.strict_modes.push(strict_mode);
        self.module_mode = matches!(program, SwcProgram::Module(_));

        match program {
            SwcProgram::Script(script) => {
                let scope_bindings = collect_direct_statement_lexical_bindings(&script.body)?;
                self.push_binding_scope(scope_bindings);
                let lowered = self.lower_top_level_statements(script.body.iter(), &mut statements);
                self.pop_binding_scope();
                lowered?
            }
            SwcProgram::Module(module) => {
                for item in &module.body {
                    match item {
                        ModuleItem::Stmt(statement) => {
                            self.lower_top_level_statement(statement, &mut statements)?
                        }
                        ModuleItem::ModuleDecl(module_declaration) => {
                            self.lower_module_declaration(module_declaration, &mut statements)?
                        }
                    }
                }
            }
        }

        self.strict_modes.pop();
        self.module_mode = false;
        self.current_module_path = None;
        self.module_index_lookup.clear();

        Ok(self.finish_program(statements, strict_mode))
    }

    fn finish_program(&mut self, statements: Vec<Statement>, strict: bool) -> Program {
        self.module_mode = false;
        self.current_module_path = None;
        self.module_index_lookup.clear();

        let mut functions = Vec::new();
        let mut seen = HashSet::new();
        for function in std::mem::take(&mut self.functions).into_iter().rev() {
            if seen.insert(function.name.clone()) {
                functions.push(function);
            }
        }
        functions.reverse();

        Program {
            strict,
            functions,
            statements,
        }
    }

    fn fresh_temporary_name(&mut self, prefix: &str) -> String {
        self.next_temporary_id += 1;
        format!("__ayy_{prefix}_{}", self.next_temporary_id)
    }

    fn fresh_scoped_binding_name(&mut self, name: &str) -> String {
        self.next_temporary_id += 1;
        format!("__ayy_scope${name}${}", self.next_temporary_id)
    }

    fn push_binding_scope(&mut self, names: Vec<String>) {
        let mut scope = BindingScope::default();

        for name in names {
            if scope.names.contains(&name) {
                continue;
            }

            if self.active_binding_counts.contains_key(&name) {
                scope
                    .renames
                    .insert(name.clone(), self.fresh_scoped_binding_name(&name));
            }

            *self.active_binding_counts.entry(name.clone()).or_insert(0) += 1;
            scope.names.push(name);
        }

        self.binding_scopes.push(scope);
    }

    fn pop_binding_scope(&mut self) {
        let Some(scope) = self.binding_scopes.pop() else {
            return;
        };

        for name in scope.names {
            let Some(count) = self.active_binding_counts.get_mut(&name) else {
                continue;
            };
            *count -= 1;
            if *count == 0 {
                self.active_binding_counts.remove(&name);
            }
        }
    }

    fn resolve_binding_name(&self, name: &str) -> String {
        for scope in self.binding_scopes.iter().rev() {
            if let Some(mapped) = scope.renames.get(name) {
                return mapped.clone();
            }
        }

        name.to_string()
    }

    fn lower_dynamic_import_expression(
        &mut self,
        call: &swc_ecma_ast::CallExpr,
    ) -> Result<Expression> {
        ensure!(
            call.args.len() == 1,
            "dynamic import expects exactly one argument"
        );
        let argument = &call.args[0];
        ensure!(
            argument.spread.is_none(),
            "dynamic import does not support spread arguments"
        );

        let Expr::Lit(Lit::Str(specifier)) = &*argument.expr else {
            bail!("unsupported dynamic import specifier");
        };
        let module_index = self
            .current_module_path
            .as_ref()
            .and_then(|module_path| {
                resolve_module_specifier(module_path, &specifier.value.to_string_lossy()).ok()
            })
            .and_then(|resolved| self.module_index_lookup.get(&resolved).copied())
            .map(|module_index| module_index as f64)
            .unwrap_or(-1.0);

        Ok(Expression::Call {
            callee: Box::new(Expression::Identifier("__ayyDynamicImport".to_string())),
            arguments: vec![CallArgument::Expression(Expression::Number(module_index))],
        })
    }

    fn lower_private_name(&self, private_name: &swc_ecma_ast::PrivateName) -> Result<Expression> {
        let name = private_name.name.to_string();
        for scope in self.private_name_scopes.iter().rev() {
            if let Some(mapped) = scope.get(&name) {
                return Ok(Expression::String(mapped.clone()));
            }
        }

        bail!("unsupported private name reference: #{name}")
    }

    fn class_private_name_map(&self, class: &Class, binding_name: &str) -> HashMap<String, String> {
        let mut names = HashMap::new();
        for member in &class.body {
            match member {
                ClassMember::PrivateProp(property) => {
                    names.insert(
                        property.key.name.to_string(),
                        format!("__ayy$private${binding_name}${}", property.key.name),
                    );
                }
                ClassMember::PrivateMethod(method) => {
                    names.insert(
                        method.key.name.to_string(),
                        format!("__ayy$private${binding_name}${}", method.key.name),
                    );
                }
                _ => {}
            }
        }
        names
    }

    fn current_strict_mode(&self) -> bool {
        self.strict_modes.last().copied().unwrap_or(false)
    }

    fn function_strict_mode(&self, function: &Function) -> bool {
        self.current_strict_mode() || function_has_use_strict_directive(function)
    }

    fn arrow_strict_mode(&self, arrow_expression: &ArrowExpr) -> bool {
        self.current_strict_mode()
            || match &*arrow_expression.body {
                BlockStmtOrExpr::BlockStmt(block) => script_has_use_strict_directive(&block.stmts),
                BlockStmtOrExpr::Expr(_) => false,
            }
    }

    fn function_has_mapped_arguments(&self, function: &Function) -> bool {
        !self.function_strict_mode(function) && function_has_simple_parameter_list(function)
    }

    fn lower_top_level_statements<'a>(
        &mut self,
        statements: impl Iterator<Item = &'a Stmt>,
        lowered_statements: &mut Vec<Statement>,
    ) -> Result<()> {
        for statement in statements {
            self.lower_top_level_statement(statement, lowered_statements)?;
        }

        Ok(())
    }

    fn lower_top_level_statement(
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

    fn lower_module_declaration(
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

    fn lower_export_default_declaration(
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

    fn lower_named_default_function_expression(
        &mut self,
        function_expression: &FnExpr,
    ) -> Result<String> {
        self.next_function_expression_id += 1;
        let identifier = function_expression
            .ident
            .as_ref()
            .context("named default export function must have an identifier")?;
        let generated_name = format!(
            "__ayy_fnstmt_{}_{}",
            identifier.sym, self.next_function_expression_id
        );
        let kind = lower_function_kind(
            function_expression.function.is_generator,
            function_expression.function.is_async,
        );
        let extra_bindings = vec![identifier.sym.to_string()];
        let (params, body) =
            self.lower_function_parts(&function_expression.function, &extra_bindings)?;

        self.functions.push(FunctionDeclaration {
            name: generated_name.clone(),
            top_level_binding: None,
            params,
            body,
            register_global: false,
            kind,
            self_binding: None,
            mapped_arguments: self.function_has_mapped_arguments(&function_expression.function),
            strict: self.function_strict_mode(&function_expression.function),
            lexical_this: false,
            length: expected_argument_count(
                function_expression
                    .function
                    .params
                    .iter()
                    .map(|parameter| &parameter.pat),
            ),
        });

        Ok(generated_name)
    }

    fn lower_function_declaration(
        &mut self,
        function_declaration: &FnDecl,
        register_global: bool,
    ) -> Result<FunctionDeclaration> {
        ensure!(
            !function_declaration.declare,
            "declare function is not supported yet"
        );

        let extra_bindings = vec![function_declaration.ident.sym.to_string()];
        let (params, body) =
            self.lower_function_parts(&function_declaration.function, &extra_bindings)?;

        Ok(FunctionDeclaration {
            name: function_declaration.ident.sym.to_string(),
            top_level_binding: None,
            params,
            body,
            register_global,
            kind: lower_function_kind(
                function_declaration.function.is_generator,
                function_declaration.function.is_async,
            ),
            self_binding: Some(function_declaration.ident.sym.to_string()),
            mapped_arguments: self.function_has_mapped_arguments(&function_declaration.function),
            strict: self.function_strict_mode(&function_declaration.function),
            lexical_this: false,
            length: expected_argument_count(
                function_declaration
                    .function
                    .params
                    .iter()
                    .map(|parameter| &parameter.pat),
            ),
        })
    }

    fn lower_function_expression(
        &mut self,
        function_expression: &FnExpr,
        name_hint: Option<&str>,
    ) -> Result<Expression> {
        self.next_function_expression_id += 1;
        let generated_name = match &function_expression.ident {
            Some(identifier) => {
                format!(
                    "__ayy_fnexpr_{}_{}",
                    identifier.sym, self.next_function_expression_id
                )
            }
            None => match name_hint {
                Some(name_hint) => format!(
                    "__ayy_fnexpr_{}__name_{}",
                    self.next_function_expression_id, name_hint
                ),
                None => format!("__ayy_fnexpr_{}", self.next_function_expression_id),
            },
        };
        let kind = lower_function_kind(
            function_expression.function.is_generator,
            function_expression.function.is_async,
        );
        let extra_bindings = function_expression
            .ident
            .as_ref()
            .map(|identifier| vec![identifier.sym.to_string()])
            .unwrap_or_default();
        let (params, body) =
            self.lower_function_parts(&function_expression.function, &extra_bindings)?;
        let self_binding = function_expression
            .ident
            .as_ref()
            .map(|identifier| identifier.sym.to_string());

        self.functions.push(FunctionDeclaration {
            name: generated_name.clone(),
            top_level_binding: None,
            params,
            body,
            register_global: false,
            kind,
            self_binding,
            mapped_arguments: self.function_has_mapped_arguments(&function_expression.function),
            strict: self.function_strict_mode(&function_expression.function),
            lexical_this: false,
            length: expected_argument_count(
                function_expression
                    .function
                    .params
                    .iter()
                    .map(|parameter| &parameter.pat),
            ),
        });

        Ok(Expression::Identifier(generated_name))
    }

    fn lower_arrow_expression(
        &mut self,
        arrow_expression: &ArrowExpr,
        name_hint: Option<&str>,
    ) -> Result<Expression> {
        self.next_function_expression_id += 1;
        let generated_name = match name_hint {
            Some(name_hint) => format!(
                "__ayy_arrow_{}__name_{}",
                self.next_function_expression_id, name_hint
            ),
            None => format!("__ayy_arrow_{}", self.next_function_expression_id),
        };

        let (params, param_setup) = lower_parameter_patterns(self, arrow_expression.params.iter())?;

        let mut body = match &*arrow_expression.body {
            BlockStmtOrExpr::BlockStmt(block) => {
                self.lower_statements(&block.stmts, true, false)?
            }
            BlockStmtOrExpr::Expr(expression) => vec![Statement::Return(
                self.lower_expression_with_name_hint(expression, None)?,
            )],
        };
        body.splice(0..0, param_setup);

        self.functions.push(FunctionDeclaration {
            name: generated_name.clone(),
            top_level_binding: None,
            params,
            body,
            register_global: false,
            kind: if arrow_expression.is_async {
                FunctionKind::Async
            } else {
                FunctionKind::Ordinary
            },
            self_binding: None,
            mapped_arguments: false,
            strict: self.arrow_strict_mode(arrow_expression),
            lexical_this: true,
            length: expected_argument_count(arrow_expression.params.iter()),
        });

        Ok(Expression::Identifier(generated_name))
    }

    fn lower_statements(
        &mut self,
        statements: &[Stmt],
        allow_return: bool,
        allow_loop_control: bool,
    ) -> Result<Vec<Statement>> {
        let scope_bindings = collect_direct_statement_lexical_bindings(statements)?;
        self.push_binding_scope(scope_bindings);
        let lowered = (|| -> Result<Vec<Statement>> {
            let mut lowered = Vec::new();

            for statement in statements {
                lowered.extend(self.lower_statement(
                    statement,
                    allow_return,
                    allow_loop_control,
                )?);
            }

            Ok(lowered)
        })();
        self.pop_binding_scope();
        lowered
    }

    fn lower_statement(
        &mut self,
        statement: &Stmt,
        allow_return: bool,
        allow_loop_control: bool,
    ) -> Result<Vec<Statement>> {
        match statement {
            Stmt::Decl(Decl::Var(variable_declaration)) => {
                self.lower_variable_declaration(variable_declaration)
            }
            Stmt::Decl(Decl::Fn(function_declaration)) => {
                self.lower_nested_function_declaration(function_declaration)
            }
            Stmt::Decl(Decl::Class(class_declaration)) => {
                self.lower_class_declaration(class_declaration)
            }
            Stmt::Expr(ExprStmt { expr, .. }) => self.lower_expression_statement(expr),
            Stmt::Block(block) => Ok(vec![Statement::Block {
                body: self.lower_statements(&block.stmts, allow_return, allow_loop_control)?,
            }]),
            Stmt::If(if_statement) => Ok(vec![Statement::If {
                condition: self.lower_expression(&if_statement.test)?,
                then_branch: self.lower_block_or_statement(
                    &if_statement.cons,
                    allow_return,
                    allow_loop_control,
                )?,
                else_branch: self.lower_optional_else(
                    if_statement.alt.as_deref(),
                    allow_return,
                    allow_loop_control,
                )?,
            }]),
            Stmt::Switch(switch_statement) => {
                self.lower_switch_statement(switch_statement, allow_return, allow_loop_control)
            }
            Stmt::For(for_statement) => Ok(vec![Statement::For {
                labels: Vec::new(),
                init: match &for_statement.init {
                    Some(VarDeclOrExpr::VarDecl(variable_declaration)) => {
                        self.lower_variable_declaration(variable_declaration)?
                    }
                    Some(VarDeclOrExpr::Expr(expression)) => {
                        self.lower_expression_statement(expression)?
                    }
                    None => Vec::new(),
                },
                condition: for_statement
                    .test
                    .as_deref()
                    .map(|expression| self.lower_expression(expression))
                    .transpose()?,
                update: for_statement
                    .update
                    .as_deref()
                    .map(|expression| self.lower_expression(expression))
                    .transpose()?,
                per_iteration_bindings: for_statement
                    .init
                    .as_ref()
                    .map(collect_for_per_iteration_bindings)
                    .transpose()?
                    .unwrap_or_default(),
                break_hook: None,
                body: self.lower_block_or_statement(&for_statement.body, allow_return, true)?,
            }]),
            Stmt::ForOf(for_of_statement) => {
                self.lower_for_of_statement(for_of_statement, allow_return)
            }
            Stmt::ForIn(for_in_statement) => {
                self.lower_for_in_statement(for_in_statement, allow_return)
            }
            Stmt::DoWhile(do_while_statement) => Ok(vec![Statement::DoWhile {
                labels: Vec::new(),
                condition: self.lower_expression(&do_while_statement.test)?,
                break_hook: None,
                body: self.lower_block_or_statement(
                    &do_while_statement.body,
                    allow_return,
                    true,
                )?,
            }]),
            Stmt::With(with_statement) => Ok(vec![Statement::With {
                object: self.lower_expression(&with_statement.obj)?,
                body: self.lower_block_or_statement(
                    &with_statement.body,
                    allow_return,
                    allow_loop_control,
                )?,
            }]),
            Stmt::While(while_statement) => Ok(vec![Statement::While {
                labels: Vec::new(),
                condition: self.lower_expression(&while_statement.test)?,
                break_hook: None,
                body: self.lower_block_or_statement(&while_statement.body, allow_return, true)?,
            }]),
            Stmt::Throw(throw_statement) => Ok(vec![Statement::Throw(
                self.lower_expression(&throw_statement.arg)?,
            )]),
            Stmt::Try(try_statement) => {
                self.lower_try_statement(try_statement, allow_return, allow_loop_control)
            }
            Stmt::Return(return_statement) => {
                ensure!(allow_return, "`return` is only supported inside functions");
                Ok(vec![Statement::Return(
                    match return_statement.arg.as_deref() {
                        Some(expression) => self.lower_expression(expression)?,
                        None => Expression::Undefined,
                    },
                )])
            }
            Stmt::Break(break_statement) => {
                self.lower_break_statement(break_statement, allow_loop_control)
            }
            Stmt::Continue(continue_statement) => {
                self.lower_continue_statement(continue_statement, allow_loop_control)
            }
            Stmt::Labeled(labeled_statement) => {
                self.lower_labeled_statement(labeled_statement, allow_return, allow_loop_control)
            }
            Stmt::Empty(_) => Ok(Vec::new()),
            _ => bail!("unsupported statement: {statement:?}"),
        }
    }

    fn lower_try_statement(
        &mut self,
        try_statement: &swc_ecma_ast::TryStmt,
        allow_return: bool,
        allow_loop_control: bool,
    ) -> Result<Vec<Statement>> {
        let lowered_body =
            self.lower_statements(&try_statement.block.stmts, allow_return, allow_loop_control)?;
        let lowered_handler = try_statement
            .handler
            .as_ref()
            .map(|handler| self.lower_catch_clause(handler, allow_return, allow_loop_control))
            .transpose()?;

        if let Some(finalizer) = &try_statement.finalizer {
            let threw_name = self.fresh_temporary_name("finally_threw");
            let error_name = self.fresh_temporary_name("finally_error");
            let outer_catch_name = self.fresh_temporary_name("finally_catch");
            let mut statements = vec![
                Statement::Let {
                    name: threw_name.clone(),
                    mutable: true,
                    value: Expression::Bool(false),
                },
                Statement::Let {
                    name: error_name.clone(),
                    mutable: true,
                    value: Expression::Undefined,
                },
            ];

            let protected_body =
                if let Some((catch_binding, catch_setup, catch_body)) = lowered_handler {
                    vec![Statement::Try {
                        body: lowered_body,
                        catch_binding,
                        catch_setup,
                        catch_body,
                    }]
                } else {
                    lowered_body
                };

            statements.push(Statement::Try {
                body: protected_body,
                catch_binding: Some(outer_catch_name.clone()),
                catch_setup: Vec::new(),
                catch_body: vec![
                    Statement::Assign {
                        name: threw_name.clone(),
                        value: Expression::Bool(true),
                    },
                    Statement::Assign {
                        name: error_name.clone(),
                        value: Expression::Identifier(outer_catch_name),
                    },
                ],
            });
            statements.extend(self.lower_statements(
                &finalizer.stmts,
                allow_return,
                allow_loop_control,
            )?);
            statements.push(Statement::If {
                condition: Expression::Identifier(threw_name),
                then_branch: vec![Statement::Throw(Expression::Identifier(error_name))],
                else_branch: Vec::new(),
            });
            return Ok(statements);
        }

        let (catch_binding, catch_setup, catch_body) =
            lowered_handler.context("`try` without `catch` is not supported yet")?;
        Ok(vec![Statement::Try {
            body: lowered_body,
            catch_binding,
            catch_setup,
            catch_body,
        }])
    }

    fn lower_catch_clause(
        &mut self,
        handler: &swc_ecma_ast::CatchClause,
        allow_return: bool,
        allow_loop_control: bool,
    ) -> Result<(Option<String>, Vec<Statement>, Vec<Statement>)> {
        let mut scope_bindings = Vec::new();
        if let Some(pattern) = handler.param.as_ref() {
            collect_pattern_binding_names(pattern, &mut scope_bindings)?;
        }

        self.push_binding_scope(scope_bindings);
        let lowered = (|| -> Result<(Option<String>, Vec<Statement>, Vec<Statement>)> {
            let (catch_binding, catch_setup) = match handler.param.as_ref() {
                Some(Pat::Ident(binding)) => (
                    Some(self.resolve_binding_name(binding.id.sym.as_ref())),
                    Vec::new(),
                ),
                None => (None, Vec::new()),
                Some(pattern) => {
                    let temporary_name = self.fresh_temporary_name("catch");
                    let mut setup = Vec::new();
                    self.lower_for_of_pattern_binding(
                        pattern,
                        Expression::Identifier(temporary_name.clone()),
                        ForOfPatternBindingKind::Lexical { mutable: true },
                        &mut setup,
                    )?;
                    (Some(temporary_name), setup)
                }
            };

            Ok((
                catch_binding,
                catch_setup,
                self.lower_statements(&handler.body.stmts, allow_return, allow_loop_control)?,
            ))
        })();
        self.pop_binding_scope();
        lowered
    }

    fn lower_generator_statements(
        &mut self,
        statements: &[Stmt],
        allow_return: bool,
    ) -> Result<Vec<Statement>> {
        let mut lowered = Vec::new();

        for statement in statements {
            lowered.extend(self.lower_generator_statement(statement, allow_return)?);
        }

        Ok(lowered)
    }

    fn lower_generator_statement(
        &mut self,
        statement: &Stmt,
        allow_return: bool,
    ) -> Result<Vec<Statement>> {
        match statement {
            Stmt::Expr(ExprStmt { expr, .. }) => {
                if let Some(lowered) = self.lower_generator_assignment_expression(expr)? {
                    return Ok(lowered);
                }

                if let Some(lowered) = self.lower_generator_effect_expression(expr)? {
                    return Ok(lowered);
                }

                self.lower_expression_statement(expr)
            }
            Stmt::Block(BlockStmt { stmts, .. })
                if stmts
                    .iter()
                    .all(|statement| matches!(statement, Stmt::Expr(_) | Stmt::Empty(_))) =>
            {
                self.lower_generator_statements(stmts, allow_return)
            }
            Stmt::For(for_statement) => Ok(vec![Statement::For {
                labels: Vec::new(),
                init: match &for_statement.init {
                    Some(VarDeclOrExpr::VarDecl(variable_declaration)) => {
                        self.lower_variable_declaration(variable_declaration)?
                    }
                    Some(VarDeclOrExpr::Expr(expression)) => {
                        self.lower_expression_statement(expression)?
                    }
                    None => Vec::new(),
                },
                condition: for_statement
                    .test
                    .as_deref()
                    .map(|expression| self.lower_expression(expression))
                    .transpose()?,
                update: for_statement
                    .update
                    .as_deref()
                    .map(|expression| self.lower_expression(expression))
                    .transpose()?,
                per_iteration_bindings: for_statement
                    .init
                    .as_ref()
                    .map(collect_for_per_iteration_bindings)
                    .transpose()?
                    .unwrap_or_default(),
                break_hook: None,
                body: self.lower_generator_loop_body(&for_statement.body, allow_return)?,
            }]),
            Stmt::ForOf(for_of_statement) => {
                self.lower_for_of_statement(for_of_statement, allow_return)
            }
            Stmt::ForIn(for_in_statement) => {
                self.lower_for_in_statement(for_in_statement, allow_return)
            }
            Stmt::If(if_statement) => Ok(vec![Statement::If {
                condition: self.lower_expression(&if_statement.test)?,
                then_branch: self.lower_generator_branch(&if_statement.cons, allow_return)?,
                else_branch: if let Some(alternate) = &if_statement.alt {
                    self.lower_generator_branch(alternate, allow_return)?
                } else {
                    Vec::new()
                },
            }]),
            Stmt::DoWhile(do_while_statement) => Ok(vec![Statement::DoWhile {
                labels: Vec::new(),
                condition: self.lower_expression(&do_while_statement.test)?,
                break_hook: None,
                body: self.lower_generator_loop_body(&do_while_statement.body, allow_return)?,
            }]),
            Stmt::Labeled(labeled_statement) => {
                self.lower_labeled_statement(labeled_statement, allow_return, false)
            }
            Stmt::With(with_statement) => {
                self.lower_generator_with_statement(with_statement, allow_return)
            }
            Stmt::Return(return_statement) => {
                ensure!(allow_return, "`return` is only supported inside functions");
                Ok(vec![Statement::Return(
                    match return_statement.arg.as_deref() {
                        Some(expression) => self.lower_expression(expression)?,
                        None => Expression::Undefined,
                    },
                )])
            }
            Stmt::Empty(_) => Ok(Vec::new()),
            other => self.lower_statement(other, allow_return, false),
        }
    }

    fn lower_generator_loop_body(
        &mut self,
        statement: &Stmt,
        allow_return: bool,
    ) -> Result<Vec<Statement>> {
        match statement {
            Stmt::Block(BlockStmt { stmts, .. }) => {
                self.lower_generator_statements(stmts, allow_return)
            }
            other => self.lower_generator_statement(other, allow_return),
        }
    }

    fn lower_generator_branch(
        &mut self,
        statement: &Stmt,
        allow_return: bool,
    ) -> Result<Vec<Statement>> {
        match statement {
            Stmt::Block(BlockStmt { stmts, .. }) => {
                self.lower_generator_statements(stmts, allow_return)
            }
            other => self.lower_generator_statement(other, allow_return),
        }
    }

    fn lower_for_of_statement(
        &mut self,
        for_of_statement: &ForOfStmt,
        allow_return: bool,
    ) -> Result<Vec<Statement>> {
        let iterator_name = self.fresh_temporary_name("for_of_iter");
        let step_name = self.fresh_temporary_name("for_of_step");
        let done_name = self.fresh_temporary_name("for_of_done");
        let iterator_value =
            Expression::GetIterator(Box::new(self.lower_expression(&for_of_statement.right)?));
        let step_value = Expression::Call {
            callee: Box::new(Expression::Member {
                object: Box::new(Expression::Identifier(iterator_name.clone())),
                property: Box::new(Expression::String("next".to_string())),
            }),
            arguments: Vec::new(),
        };
        let step_done = Expression::Member {
            object: Box::new(Expression::Identifier(step_name.clone())),
            property: Box::new(Expression::String("done".to_string())),
        };
        let iterated_value = Expression::Member {
            object: Box::new(Expression::Identifier(step_name.clone())),
            property: Box::new(Expression::String("value".to_string())),
        };
        let break_hook = Expression::Conditional {
            condition: Box::new(Expression::Identifier(done_name.clone())),
            then_expression: Box::new(Expression::Undefined),
            else_expression: Box::new(Expression::IteratorClose(Box::new(Expression::Identifier(
                iterator_name.clone(),
            )))),
        };
        let binding = self.lower_for_of_binding(&for_of_statement.left, iterated_value)?;

        let mut body = vec![
            Statement::Let {
                name: step_name,
                mutable: true,
                value: step_value,
            },
            Statement::If {
                condition: step_done,
                then_branch: vec![
                    Statement::Assign {
                        name: done_name.clone(),
                        value: Expression::Bool(true),
                    },
                    Statement::Break { label: None },
                ],
                else_branch: Vec::new(),
            },
        ];
        body.extend(binding.per_iteration);
        body.extend(self.lower_block_or_statement(&for_of_statement.body, allow_return, true)?);

        let mut lowered = vec![Statement::Let {
            name: iterator_name,
            mutable: true,
            value: iterator_value,
        }];
        lowered.extend(binding.before_loop);
        lowered.push(Statement::Let {
            name: done_name,
            mutable: true,
            value: Expression::Bool(false),
        });
        lowered.push(Statement::While {
            labels: Vec::new(),
            condition: Expression::Bool(true),
            break_hook: Some(break_hook),
            body,
        });
        Ok(lowered)
    }

    fn lower_for_in_statement(
        &mut self,
        for_in_statement: &ForInStmt,
        allow_return: bool,
    ) -> Result<Vec<Statement>> {
        let target_name = self.fresh_temporary_name("for_in_target");
        let keys_name = self.fresh_temporary_name("for_in_keys");
        let index_name = self.fresh_temporary_name("for_in_index");
        let target_value = self.lower_expression(&for_in_statement.right)?;
        let target_expression = Expression::Identifier(target_name.clone());
        let enumerated_keys = Expression::EnumerateKeys(Box::new(target_expression.clone()));
        let current_key = Expression::Member {
            object: Box::new(Expression::Identifier(keys_name.clone())),
            property: Box::new(Expression::Identifier(index_name.clone())),
        };
        let binding = self.lower_for_of_binding(&for_in_statement.left, current_key.clone())?;

        let mut init = binding.before_loop;
        init.push(Statement::Let {
            name: target_name,
            mutable: false,
            value: target_value,
        });
        init.push(Statement::Let {
            name: keys_name.clone(),
            mutable: false,
            value: enumerated_keys,
        });
        init.push(Statement::Let {
            name: index_name.clone(),
            mutable: true,
            value: Expression::Number(0.0),
        });

        let mut body = vec![Statement::If {
            condition: Expression::Unary {
                op: UnaryOp::Not,
                expression: Box::new(Expression::Binary {
                    op: BinaryOp::In,
                    left: Box::new(current_key),
                    right: Box::new(target_expression),
                }),
            },
            then_branch: vec![Statement::Continue { label: None }],
            else_branch: Vec::new(),
        }];
        body.extend(binding.per_iteration);
        body.extend(self.lower_block_or_statement(&for_in_statement.body, allow_return, true)?);

        Ok(vec![Statement::For {
            labels: Vec::new(),
            init,
            per_iteration_bindings: Vec::new(),
            condition: Some(Expression::Binary {
                op: BinaryOp::LessThan,
                left: Box::new(Expression::Identifier(index_name.clone())),
                right: Box::new(Expression::Member {
                    object: Box::new(Expression::Identifier(keys_name)),
                    property: Box::new(Expression::String("length".to_string())),
                }),
            }),
            update: Some(Expression::Update {
                name: index_name,
                op: UpdateOp::Increment,
                prefix: false,
            }),
            break_hook: None,
            body,
        }])
    }

    fn lower_break_statement(
        &mut self,
        break_statement: &BreakStmt,
        allow_loop_control: bool,
    ) -> Result<Vec<Statement>> {
        if break_statement.label.is_none() {
            ensure!(allow_loop_control, "`break` is only supported inside loops");
        }

        Ok(vec![Statement::Break {
            label: break_statement
                .label
                .as_ref()
                .map(|label| label.sym.to_string()),
        }])
    }

    fn lower_continue_statement(
        &mut self,
        continue_statement: &ContinueStmt,
        allow_loop_control: bool,
    ) -> Result<Vec<Statement>> {
        if continue_statement.label.is_none() {
            ensure!(
                allow_loop_control,
                "`continue` is only supported inside loops"
            );
        }

        Ok(vec![Statement::Continue {
            label: continue_statement
                .label
                .as_ref()
                .map(|label| label.sym.to_string()),
        }])
    }

    fn lower_switch_statement(
        &mut self,
        switch_statement: &SwitchStmt,
        allow_return: bool,
        _allow_loop_control: bool,
    ) -> Result<Vec<Statement>> {
        let bindings = collect_switch_bindings(switch_statement)?;
        let binding_names = bindings.iter().cloned().collect::<HashSet<_>>();
        let cases = switch_statement
            .cases
            .iter()
            .map(|case| {
                Ok(SwitchCase {
                    test: case
                        .test
                        .as_deref()
                        .map(|expression| self.lower_expression(expression))
                        .transpose()?,
                    body: self.lower_switch_case_statements(
                        &case.cons,
                        allow_return,
                        true,
                        &binding_names,
                    )?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(vec![Statement::Switch {
            labels: Vec::new(),
            bindings,
            discriminant: self.lower_expression(&switch_statement.discriminant)?,
            cases,
        }])
    }

    fn lower_switch_case_statements(
        &mut self,
        statements: &[Stmt],
        allow_return: bool,
        allow_loop_control: bool,
        bindings: &HashSet<String>,
    ) -> Result<Vec<Statement>> {
        let mut lowered = Vec::new();

        for statement in statements {
            if let Stmt::Decl(Decl::Var(variable_declaration)) = statement
                && !matches!(variable_declaration.kind, VarDeclKind::Var)
            {
                lowered.extend(
                    self.lower_switch_case_lexical_declaration(variable_declaration, bindings)?,
                );
                continue;
            }

            lowered.extend(self.lower_statement(statement, allow_return, allow_loop_control)?);
        }

        Ok(lowered)
    }

    fn lower_switch_case_lexical_declaration(
        &mut self,
        variable_declaration: &swc_ecma_ast::VarDecl,
        bindings: &HashSet<String>,
    ) -> Result<Vec<Statement>> {
        let mut lowered = Vec::new();

        for declarator in &variable_declaration.decls {
            let mut names = Vec::new();
            collect_pattern_binding_names(&declarator.name, &mut names)?;
            if names.iter().any(|name| !bindings.contains(name)) {
                bail!("unsupported switch lexical binding");
            }

            let value = match declarator.init.as_deref() {
                Some(initializer) => self.lower_expression_with_name_hint(
                    initializer,
                    pattern_name_hint(&declarator.name),
                )?,
                None => Expression::Undefined,
            };

            if let Pat::Ident(identifier) = &declarator.name {
                lowered.push(Statement::Assign {
                    name: identifier.id.sym.to_string(),
                    value,
                });
                continue;
            }

            let temporary_name = self.fresh_temporary_name("switch_decl");
            lowered.push(Statement::Let {
                name: temporary_name.clone(),
                mutable: true,
                value,
            });
            self.lower_for_of_pattern_binding(
                &declarator.name,
                Expression::Identifier(temporary_name),
                ForOfPatternBindingKind::Assignment,
                &mut lowered,
            )?;
        }

        Ok(lowered)
    }

    fn lower_labeled_statement(
        &mut self,
        labeled_statement: &LabeledStmt,
        allow_return: bool,
        allow_loop_control: bool,
    ) -> Result<Vec<Statement>> {
        let label = labeled_statement.label.sym.to_string();
        let mut lowered = match &*labeled_statement.body {
            Stmt::Block(block) => vec![Statement::Labeled {
                labels: Vec::new(),
                body: self.lower_statements(&block.stmts, allow_return, allow_loop_control)?,
            }],
            statement => self.lower_statement(statement, allow_return, allow_loop_control)?,
        };

        self.attach_label_to_lowered(&mut lowered, label)?;
        Ok(lowered)
    }

    fn attach_label_to_lowered(
        &mut self,
        lowered: &mut Vec<Statement>,
        label: String,
    ) -> Result<()> {
        let single_statement = lowered.len() == 1;
        if let Some(last) = lowered.last_mut() {
            match last {
                Statement::For { labels, .. }
                | Statement::While { labels, .. }
                | Statement::DoWhile { labels, .. }
                | Statement::Switch { labels, .. } => {
                    labels.insert(0, label);
                    return Ok(());
                }
                Statement::Labeled { labels, .. } if single_statement => {
                    labels.insert(0, label);
                    return Ok(());
                }
                _ => {}
            }
        }

        if lowered.is_empty() {
            bail!("unsupported labeled statement")
        }

        let body = std::mem::take(lowered);
        lowered.push(Statement::Labeled {
            labels: vec![label],
            body,
        });
        Ok(())
    }

    fn lower_generator_with_statement(
        &mut self,
        with_statement: &WithStmt,
        allow_return: bool,
    ) -> Result<Vec<Statement>> {
        let Expr::Object(object) = &*with_statement.obj else {
            bail!("only object literal `with` is supported in generator functions")
        };

        let mut bindings = HashMap::new();
        for property in &object.props {
            match property {
                PropOrSpread::Prop(property) => match &**property {
                    Prop::KeyValue(property) => {
                        let key = match &property.key {
                            PropName::Ident(identifier) => identifier.sym.to_string(),
                            PropName::Str(string) => string.value.to_string_lossy().into_owned(),
                            _ => bail!("unsupported `with` property key"),
                        };
                        bindings.insert(key, self.lower_expression(&property.value)?);
                    }
                    _ => bail!("unsupported `with` object property"),
                },
                PropOrSpread::Spread(_) => bail!("unsupported `with` object spread"),
            }
        }

        self.lower_generator_with_body(&with_statement.body, allow_return, &bindings)
    }

    fn lower_generator_with_body(
        &mut self,
        statement: &Stmt,
        allow_return: bool,
        bindings: &HashMap<String, Expression>,
    ) -> Result<Vec<Statement>> {
        match statement {
            Stmt::Block(BlockStmt { stmts, .. }) => {
                let mut lowered = Vec::new();
                for statement in stmts {
                    lowered.extend(self.lower_generator_with_body(
                        statement,
                        allow_return,
                        bindings,
                    )?);
                }
                Ok(lowered)
            }
            Stmt::Expr(ExprStmt { expr, .. }) => {
                self.lower_generator_with_expression(expr, bindings)
            }
            Stmt::Empty(_) => Ok(Vec::new()),
            Stmt::Return(return_statement) => {
                ensure!(allow_return, "`return` is only supported inside functions");
                Ok(vec![Statement::Return(
                    match return_statement.arg.as_deref() {
                        Some(expression) => {
                            self.lower_expression_with_generator_bindings(expression, bindings)?
                        }
                        None => Expression::Undefined,
                    },
                )])
            }
            _ => bail!("unsupported statement inside generator `with`"),
        }
    }

    fn lower_generator_with_expression(
        &mut self,
        expression: &Expr,
        bindings: &HashMap<String, Expression>,
    ) -> Result<Vec<Statement>> {
        let Expr::Yield(yield_expression) = expression else {
            bail!("unsupported expression inside generator `with`")
        };

        if yield_expression.delegate {
            let value = yield_expression
                .arg
                .as_deref()
                .context("`yield*` requires an operand")?;
            return Ok(vec![Statement::YieldDelegate {
                value: self.lower_expression_with_generator_bindings(value, bindings)?,
            }]);
        }

        match yield_expression.arg.as_deref() {
            Some(Expr::Yield(inner_yield)) => {
                ensure!(
                    !inner_yield.delegate,
                    "`yield*` as the operand of another `yield` is not supported yet"
                );
                Ok(vec![
                    Statement::Yield {
                        value: match inner_yield.arg.as_deref() {
                            Some(value) => {
                                self.lower_expression_with_generator_bindings(value, bindings)?
                            }
                            None => Expression::Undefined,
                        },
                    },
                    Statement::Yield {
                        value: Expression::Sent,
                    },
                ])
            }
            Some(value) => Ok(vec![Statement::Yield {
                value: self.lower_expression_with_generator_bindings(value, bindings)?,
            }]),
            None => Ok(vec![Statement::Yield {
                value: Expression::Undefined,
            }]),
        }
    }

    fn lower_for_of_binding(&mut self, head: &ForHead, value: Expression) -> Result<ForOfBinding> {
        match head {
            ForHead::VarDecl(variable_declaration) => {
                ensure!(
                    variable_declaration.decls.len() == 1,
                    "for-of declarations with multiple bindings are not supported yet"
                );
                let pattern = &variable_declaration.decls[0].name;
                let mut binding = ForOfBinding {
                    before_loop: Vec::new(),
                    per_iteration: Vec::new(),
                };
                let binding_kind = match variable_declaration.kind {
                    VarDeclKind::Var => ForOfPatternBindingKind::Var,
                    VarDeclKind::Let => ForOfPatternBindingKind::Lexical { mutable: true },
                    VarDeclKind::Const => ForOfPatternBindingKind::Lexical { mutable: false },
                };

                if matches!(variable_declaration.kind, VarDeclKind::Var) {
                    let mut names = Vec::new();
                    collect_for_of_binding_names(pattern, &mut names)?;
                    binding.before_loop = names
                        .into_iter()
                        .map(|name| Statement::Var {
                            name,
                            value: Expression::Undefined,
                        })
                        .collect();
                }

                self.lower_for_of_pattern_binding(
                    pattern,
                    value,
                    binding_kind,
                    &mut binding.per_iteration,
                )?;

                Ok(binding)
            }
            ForHead::Pat(pattern) => {
                let mut binding = ForOfBinding {
                    before_loop: Vec::new(),
                    per_iteration: Vec::new(),
                };
                self.lower_for_of_pattern_binding(
                    pattern,
                    value,
                    ForOfPatternBindingKind::Assignment,
                    &mut binding.per_iteration,
                )?;
                Ok(binding)
            }
            ForHead::UsingDecl(_) => bail!("using declarations are not supported yet"),
        }
    }

    fn lower_for_of_pattern_binding(
        &mut self,
        pattern: &Pat,
        value: Expression,
        binding_kind: ForOfPatternBindingKind,
        statements: &mut Vec<Statement>,
    ) -> Result<()> {
        match pattern {
            Pat::Ident(identifier) => {
                let name = self.resolve_binding_name(identifier.id.sym.as_ref());
                statements.push(match binding_kind {
                    ForOfPatternBindingKind::Var => Statement::Var { name, value },
                    ForOfPatternBindingKind::Assignment => Statement::Assign { name, value },
                    ForOfPatternBindingKind::Lexical { mutable } => Statement::Let {
                        name,
                        mutable,
                        value,
                    },
                })
            }
            Pat::Expr(expression) => {
                ensure!(
                    matches!(binding_kind, ForOfPatternBindingKind::Assignment),
                    "unsupported declaration binding pattern"
                );
                let target = self.lower_for_of_expression_target(expression)?;
                statements.push(target.into_statement(value));
            }
            Pat::Assign(assign) => {
                let temporary_name = self.fresh_temporary_name("binding_value");
                statements.push(Statement::Let {
                    name: temporary_name.clone(),
                    mutable: true,
                    value,
                });
                let mut then_branch = Vec::new();
                self.lower_for_of_pattern_binding(
                    &assign.left,
                    Expression::Identifier(temporary_name.clone()),
                    binding_kind,
                    &mut then_branch,
                )?;
                let mut else_branch = Vec::new();
                let default_value = self.lower_expression_with_name_hint(
                    &assign.right,
                    pattern_name_hint(&assign.left),
                )?;
                self.lower_for_of_pattern_binding(
                    &assign.left,
                    default_value,
                    binding_kind,
                    &mut else_branch,
                )?;
                statements.push(Statement::If {
                    condition: Expression::Binary {
                        op: BinaryOp::NotEqual,
                        left: Box::new(Expression::Identifier(temporary_name)),
                        right: Box::new(Expression::Undefined),
                    },
                    then_branch,
                    else_branch,
                });
            }
            Pat::Array(array) => {
                let has_rest = array
                    .elems
                    .iter()
                    .flatten()
                    .any(|element| matches!(element, Pat::Rest(_)));
                if !has_rest {
                    let pure_elision_count = self.pure_array_pattern_elision_count(array);
                    let iterator_name = self.fresh_temporary_name("array_iter");
                    let iterator_done_name = self.fresh_temporary_name("array_iter_done");
                    statements.push(Statement::Let {
                        name: iterator_name.clone(),
                        mutable: true,
                        value: Expression::GetIterator(Box::new(value.clone())),
                    });
                    statements.push(Statement::Let {
                        name: iterator_done_name.clone(),
                        mutable: true,
                        value: Expression::Bool(false),
                    });

                    if array.elems.is_empty() && pure_elision_count > 0 {
                        for _ in 0..pure_elision_count {
                            let step_name = self.fresh_temporary_name("array_step");
                            statements.push(Statement::Let {
                                name: step_name.clone(),
                                mutable: true,
                                value: Expression::Call {
                                    callee: Box::new(Expression::Member {
                                        object: Box::new(Expression::Identifier(
                                            iterator_name.clone(),
                                        )),
                                        property: Box::new(Expression::String("next".to_string())),
                                    }),
                                    arguments: Vec::new(),
                                },
                            });
                            statements.push(Statement::Assign {
                                name: iterator_done_name.clone(),
                                value: Expression::Member {
                                    object: Box::new(Expression::Identifier(step_name)),
                                    property: Box::new(Expression::String("done".to_string())),
                                },
                            });
                        }
                        statements.push(Statement::If {
                            condition: Expression::Binary {
                                op: BinaryOp::Equal,
                                left: Box::new(Expression::Identifier(iterator_done_name)),
                                right: Box::new(Expression::Bool(false)),
                            },
                            then_branch: vec![Statement::Expression(Expression::IteratorClose(
                                Box::new(Expression::Identifier(iterator_name)),
                            ))],
                            else_branch: Vec::new(),
                        });
                        return Ok(());
                    }

                    for element in &array.elems {
                        let step_name = self.fresh_temporary_name("array_step");
                        statements.push(Statement::Let {
                            name: step_name.clone(),
                            mutable: true,
                            value: Expression::Call {
                                callee: Box::new(Expression::Member {
                                    object: Box::new(Expression::Identifier(iterator_name.clone())),
                                    property: Box::new(Expression::String("next".to_string())),
                                }),
                                arguments: Vec::new(),
                            },
                        });
                        let step_done = Expression::Member {
                            object: Box::new(Expression::Identifier(step_name.clone())),
                            property: Box::new(Expression::String("done".to_string())),
                        };
                        statements.push(Statement::Assign {
                            name: iterator_done_name.clone(),
                            value: step_done.clone(),
                        });
                        let step_value = Expression::Conditional {
                            condition: Box::new(Expression::Binary {
                                op: BinaryOp::Equal,
                                left: Box::new(step_done),
                                right: Box::new(Expression::Bool(false)),
                            }),
                            then_expression: Box::new(Expression::Member {
                                object: Box::new(Expression::Identifier(step_name)),
                                property: Box::new(Expression::String("value".to_string())),
                            }),
                            else_expression: Box::new(Expression::Undefined),
                        };

                        if let Some(element) = element {
                            self.lower_for_of_pattern_binding(
                                element,
                                step_value,
                                binding_kind,
                                statements,
                            )?;
                        }
                    }

                    statements.push(Statement::If {
                        condition: Expression::Binary {
                            op: BinaryOp::Equal,
                            left: Box::new(Expression::Identifier(iterator_done_name)),
                            right: Box::new(Expression::Bool(false)),
                        },
                        then_branch: vec![Statement::Expression(Expression::IteratorClose(
                            Box::new(Expression::Identifier(iterator_name)),
                        ))],
                        else_branch: Vec::new(),
                    });
                    return Ok(());
                }

                let iterator_name = self.fresh_temporary_name("array_iter");
                let iterator_done_name = self.fresh_temporary_name("array_iter_done");
                statements.push(Statement::Let {
                    name: iterator_name.clone(),
                    mutable: true,
                    value: Expression::GetIterator(Box::new(value.clone())),
                });
                statements.push(Statement::Let {
                    name: iterator_done_name.clone(),
                    mutable: true,
                    value: Expression::Bool(false),
                });

                for element in &array.elems {
                    if let Some(Pat::Rest(rest)) = element {
                        let rest_array_name = self.fresh_temporary_name("array_rest");
                        let rest_step_name = self.fresh_temporary_name("array_step");
                        statements.push(Statement::Let {
                            name: rest_array_name.clone(),
                            mutable: true,
                            value: Expression::Array(Vec::new()),
                        });
                        statements.push(Statement::While {
                            labels: Vec::new(),
                            condition: Expression::Binary {
                                op: BinaryOp::Equal,
                                left: Box::new(Expression::Identifier(iterator_done_name.clone())),
                                right: Box::new(Expression::Bool(false)),
                            },
                            break_hook: None,
                            body: vec![
                                Statement::Let {
                                    name: rest_step_name.clone(),
                                    mutable: true,
                                    value: Expression::Call {
                                        callee: Box::new(Expression::Member {
                                            object: Box::new(Expression::Identifier(
                                                iterator_name.clone(),
                                            )),
                                            property: Box::new(Expression::String(
                                                "next".to_string(),
                                            )),
                                        }),
                                        arguments: Vec::new(),
                                    },
                                },
                                Statement::Assign {
                                    name: iterator_done_name.clone(),
                                    value: Expression::Member {
                                        object: Box::new(Expression::Identifier(
                                            rest_step_name.clone(),
                                        )),
                                        property: Box::new(Expression::String("done".to_string())),
                                    },
                                },
                                Statement::If {
                                    condition: Expression::Binary {
                                        op: BinaryOp::Equal,
                                        left: Box::new(Expression::Identifier(
                                            iterator_done_name.clone(),
                                        )),
                                        right: Box::new(Expression::Bool(false)),
                                    },
                                    then_branch: vec![Statement::Expression(Expression::Call {
                                        callee: Box::new(Expression::Member {
                                            object: Box::new(Expression::Identifier(
                                                rest_array_name.clone(),
                                            )),
                                            property: Box::new(Expression::String(
                                                "push".to_string(),
                                            )),
                                        }),
                                        arguments: vec![CallArgument::Expression(
                                            Expression::Member {
                                                object: Box::new(Expression::Identifier(
                                                    rest_step_name.clone(),
                                                )),
                                                property: Box::new(Expression::String(
                                                    "value".to_string(),
                                                )),
                                            },
                                        )],
                                    })],
                                    else_branch: Vec::new(),
                                },
                            ],
                        });
                        self.lower_for_of_pattern_binding(
                            &rest.arg,
                            Expression::Identifier(rest_array_name),
                            binding_kind,
                            statements,
                        )?;
                        break;
                    }

                    let step_name = self.fresh_temporary_name("array_step");
                    statements.push(Statement::Let {
                        name: step_name.clone(),
                        mutable: true,
                        value: Expression::Call {
                            callee: Box::new(Expression::Member {
                                object: Box::new(Expression::Identifier(iterator_name.clone())),
                                property: Box::new(Expression::String("next".to_string())),
                            }),
                            arguments: Vec::new(),
                        },
                    });
                    let step_done = Expression::Member {
                        object: Box::new(Expression::Identifier(step_name.clone())),
                        property: Box::new(Expression::String("done".to_string())),
                    };
                    statements.push(Statement::Assign {
                        name: iterator_done_name.clone(),
                        value: step_done.clone(),
                    });
                    let step_value = Expression::Conditional {
                        condition: Box::new(Expression::Binary {
                            op: BinaryOp::Equal,
                            left: Box::new(step_done),
                            right: Box::new(Expression::Bool(false)),
                        }),
                        then_expression: Box::new(Expression::Member {
                            object: Box::new(Expression::Identifier(step_name)),
                            property: Box::new(Expression::String("value".to_string())),
                        }),
                        else_expression: Box::new(Expression::Undefined),
                    };

                    if let Some(element) = element {
                        self.lower_for_of_pattern_binding(
                            element,
                            step_value,
                            binding_kind,
                            statements,
                        )?;
                    }
                }

                statements.push(Statement::If {
                    condition: Expression::Binary {
                        op: BinaryOp::Equal,
                        left: Box::new(Expression::Identifier(iterator_done_name)),
                        right: Box::new(Expression::Bool(false)),
                    },
                    then_branch: vec![Statement::Expression(Expression::IteratorClose(Box::new(
                        Expression::Identifier(iterator_name),
                    )))],
                    else_branch: Vec::new(),
                });
            }
            Pat::Object(object) => {
                self.emit_require_object_coercible_check(&value, statements);
                for property in &object.props {
                    match property {
                        ObjectPatProp::KeyValue(property) => {
                            let property_value = Expression::Member {
                                object: Box::new(value.clone()),
                                property: Box::new(self.lower_prop_name(&property.key)?),
                            };
                            self.lower_for_of_pattern_binding(
                                &property.value,
                                property_value,
                                binding_kind,
                                statements,
                            )?;
                        }
                        ObjectPatProp::Assign(property) => {
                            let binding_name =
                                self.resolve_binding_name(property.key.id.sym.as_ref());
                            let property_value = Expression::Member {
                                object: Box::new(value.clone()),
                                property: Box::new(Expression::String(
                                    property.key.id.sym.to_string(),
                                )),
                            };
                            let property_value = if let Some(default) = &property.value {
                                let default_value = self.lower_expression_with_name_hint(
                                    default,
                                    Some(binding_name.as_str()),
                                )?;
                                Expression::Conditional {
                                    condition: Box::new(Expression::Binary {
                                        op: BinaryOp::NotEqual,
                                        left: Box::new(property_value.clone()),
                                        right: Box::new(Expression::Undefined),
                                    }),
                                    then_expression: Box::new(property_value),
                                    else_expression: Box::new(default_value),
                                }
                            } else {
                                property_value
                            };
                            statements.push(match binding_kind {
                                ForOfPatternBindingKind::Var => Statement::Var {
                                    name: binding_name,
                                    value: property_value,
                                },
                                ForOfPatternBindingKind::Assignment => Statement::Assign {
                                    name: binding_name,
                                    value: property_value,
                                },
                                ForOfPatternBindingKind::Lexical { mutable } => Statement::Let {
                                    name: binding_name,
                                    mutable,
                                    value: property_value,
                                },
                            });
                        }
                        ObjectPatProp::Rest(_) => bail!("unsupported for-of binding pattern"),
                    }
                }
            }
            _ => bail!("unsupported for-of binding pattern"),
        }

        Ok(())
    }

    fn emit_require_object_coercible_check(
        &mut self,
        value: &Expression,
        statements: &mut Vec<Statement>,
    ) {
        let is_nullish = Expression::Binary {
            op: BinaryOp::LogicalOr,
            left: Box::new(Expression::Binary {
                op: BinaryOp::Equal,
                left: Box::new(value.clone()),
                right: Box::new(Expression::Null),
            }),
            right: Box::new(Expression::Binary {
                op: BinaryOp::Equal,
                left: Box::new(value.clone()),
                right: Box::new(Expression::Undefined),
            }),
        };

        statements.push(Statement::If {
            condition: is_nullish,
            then_branch: vec![Statement::Throw(Expression::New {
                callee: Box::new(Expression::Identifier("TypeError".to_string())),
                arguments: Vec::new(),
            })],
            else_branch: Vec::new(),
        });
    }

    fn lower_for_of_expression_target(&mut self, expression: &Expr) -> Result<AssignmentTarget> {
        match expression {
            Expr::Ident(identifier) => Ok(AssignmentTarget::Identifier(
                self.resolve_binding_name(identifier.sym.as_ref()),
            )),
            Expr::Member(member) => Ok(AssignmentTarget::Member {
                object: self.lower_expression(&member.obj)?,
                property: self.lower_member_property(&member.prop)?,
            }),
            Expr::Paren(parenthesized) => self.lower_for_of_expression_target(&parenthesized.expr),
            _ => bail!("unsupported for-of assignment target"),
        }
    }

    fn lower_expression_with_generator_bindings(
        &mut self,
        expression: &Expr,
        bindings: &HashMap<String, Expression>,
    ) -> Result<Expression> {
        match expression {
            Expr::Ident(identifier) => Ok(bindings
                .get(identifier.sym.as_ref())
                .cloned()
                .unwrap_or(Expression::Identifier(identifier.sym.to_string()))),
            _ => self.lower_expression(expression),
        }
    }

    fn lower_generator_assignment_expression(
        &mut self,
        expression: &Expr,
    ) -> Result<Option<Vec<Statement>>> {
        let Expr::Assign(assignment) = expression else {
            return Ok(None);
        };

        if assignment.op != AssignOp::Assign {
            return Ok(None);
        }

        let Some((mut lowered, value)) =
            self.lower_generator_assignment_value(&assignment.right)?
        else {
            return Ok(None);
        };

        let target = self.lower_assignment_target(&assignment.left)?;
        lowered.push(target.into_statement(value));
        Ok(Some(lowered))
    }

    fn lower_generator_assignment_value(
        &mut self,
        expression: &Expr,
    ) -> Result<Option<(Vec<Statement>, Expression)>> {
        match expression {
            Expr::Yield(yield_expression) => Ok(Some((
                self.lower_generator_yield_statement(yield_expression)?,
                Expression::Sent,
            ))),
            Expr::Paren(parenthesized) => {
                self.lower_generator_assignment_value(&parenthesized.expr)
            }
            Expr::Tpl(template) => self.lower_generator_template_value(template),
            _ => Ok(None),
        }
    }

    fn lower_generator_effect_expression(
        &mut self,
        expression: &Expr,
    ) -> Result<Option<Vec<Statement>>> {
        match expression {
            Expr::Yield(yield_expression) => Ok(Some(
                self.lower_generator_yield_statement(yield_expression)?,
            )),
            Expr::Paren(parenthesized) => {
                self.lower_generator_effect_expression(&parenthesized.expr)
            }
            Expr::Seq(sequence) => {
                let mut lowered = Vec::new();
                let mut handled = false;
                for expression in &sequence.exprs {
                    if let Some(mut expression_lowered) =
                        self.lower_generator_effect_expression(expression)?
                    {
                        lowered.append(&mut expression_lowered);
                        handled = true;
                    } else {
                        lowered.extend(self.lower_expression_statement(expression)?);
                    }
                }
                Ok(handled.then_some(lowered))
            }
            Expr::Array(array) => {
                let mut lowered = Vec::new();
                let mut handled = false;
                for element in array.elems.iter().flatten() {
                    if let Some(mut expression_lowered) =
                        self.lower_generator_effect_expression(&element.expr)?
                    {
                        lowered.append(&mut expression_lowered);
                        handled = true;
                    } else {
                        lowered.extend(self.lower_expression_statement(&element.expr)?);
                    }
                }
                Ok(handled.then_some(lowered))
            }
            Expr::Cond(conditional) => {
                let Some((mut lowered, condition)) =
                    self.lower_generator_assignment_value(&conditional.test)?
                else {
                    return Ok(None);
                };
                let then_expression = self.lower_generator_effect_yield_value(&conditional.cons)?;
                let else_expression = self.lower_generator_effect_yield_value(&conditional.alt)?;
                lowered.push(Statement::Yield {
                    value: Expression::Conditional {
                        condition: Box::new(condition),
                        then_expression: Box::new(then_expression),
                        else_expression: Box::new(else_expression),
                    },
                });
                Ok(Some(lowered))
            }
            Expr::Bin(binary) => {
                let left_lowered = self.lower_generator_assignment_value(&binary.left)?;
                let right_lowered = self.lower_generator_assignment_value(&binary.right)?;

                if left_lowered.is_none() && right_lowered.is_none() {
                    return Ok(None);
                }

                let mut lowered = Vec::new();
                let mut left = match left_lowered {
                    Some((mut statements, expression)) => {
                        lowered.append(&mut statements);
                        expression
                    }
                    None => self.lower_expression(&binary.left)?,
                };

                if right_lowered.is_some() {
                    let temporary = self.fresh_temporary_name("generator_bin_left");
                    lowered.push(Statement::Let {
                        name: temporary.clone(),
                        mutable: false,
                        value: left,
                    });
                    left = Expression::Identifier(temporary);
                }

                let right = match right_lowered {
                    Some((mut statements, expression)) => {
                        lowered.append(&mut statements);
                        expression
                    }
                    None => self.lower_expression(&binary.right)?,
                };

                lowered.push(Statement::Expression(Expression::Binary {
                    op: lower_binary_operator(binary.op)?,
                    left: Box::new(left),
                    right: Box::new(right),
                }));
                Ok(Some(lowered))
            }
            _ => Ok(None),
        }
    }

    fn lower_generator_effect_yield_value(&mut self, expression: &Expr) -> Result<Expression> {
        let Expr::Yield(yield_expression) = expression else {
            bail!("unsupported generator effect expression")
        };
        ensure!(
            !yield_expression.delegate,
            "`yield*` is not supported in generator effect branches yet"
        );
        match yield_expression.arg.as_deref() {
            Some(value) => self.lower_expression(value),
            None => Ok(Expression::Undefined),
        }
    }

    fn lower_generator_template_value(
        &mut self,
        template: &swc_ecma_ast::Tpl,
    ) -> Result<Option<(Vec<Statement>, Expression)>> {
        let mut yield_index = None;
        let mut yield_expression = None;

        for (index, expression) in template.exprs.iter().enumerate() {
            if let Expr::Yield(candidate) = &**expression {
                ensure!(
                    yield_index.is_none(),
                    "multiple yield expressions in template literals are not supported yet"
                );
                yield_index = Some(index);
                yield_expression = Some(candidate);
            }
        }

        let Some(yield_index) = yield_index else {
            return Ok(None);
        };

        let lowered = self.lower_generator_yield_statement(
            yield_expression.expect("yield expression must exist"),
        )?;
        let expression = self.lower_template_expression_with_substitution(
            template,
            yield_index,
            Expression::Sent,
        )?;
        Ok(Some((lowered, expression)))
    }

    fn lower_generator_yield_statement(
        &mut self,
        yield_expression: &swc_ecma_ast::YieldExpr,
    ) -> Result<Vec<Statement>> {
        if yield_expression.delegate {
            let value = yield_expression
                .arg
                .as_deref()
                .context("`yield*` requires an operand")?;
            return Ok(vec![Statement::YieldDelegate {
                value: self.lower_expression(value)?,
            }]);
        }

        match yield_expression.arg.as_deref() {
            None => Ok(vec![Statement::Yield {
                value: Expression::Undefined,
            }]),
            Some(Expr::Yield(inner_yield)) => {
                ensure!(
                    !inner_yield.delegate,
                    "`yield*` as the operand of another `yield` is not supported yet"
                );
                Ok(vec![
                    Statement::Yield {
                        value: match inner_yield.arg.as_deref() {
                            Some(value) => self.lower_expression(value)?,
                            None => Expression::Undefined,
                        },
                    },
                    Statement::Yield {
                        value: Expression::Sent,
                    },
                ])
            }
            Some(value) => Ok(vec![Statement::Yield {
                value: self.lower_expression(value)?,
            }]),
        }
    }

    fn lower_expression_statement(&mut self, expression: &Expr) -> Result<Vec<Statement>> {
        if let Some(arguments) = console_log_arguments(expression) {
            return Ok(vec![Statement::Print {
                values: arguments
                    .iter()
                    .map(|argument| self.lower_expression(&argument.expr))
                    .collect::<Result<Vec<_>>>()?,
            }]);
        }

        if let Some(call) = assert_throws_call(expression) {
            return self.lower_assert_throws_statement(call);
        }

        if let Expr::Assign(assignment) = expression {
            let target = self.lower_assignment_target(&assignment.left)?;

            if assignment.op == AssignOp::Assign {
                let value = match &target {
                    AssignmentTarget::Identifier(name) => {
                        self.lower_expression_with_name_hint(&assignment.right, Some(name))?
                    }
                    AssignmentTarget::Member { .. } | AssignmentTarget::SuperMember { .. } => {
                        self.lower_expression(&assignment.right)?
                    }
                };
                return Ok(vec![target.into_statement(value)]);
            }

            let operator = assignment
                .op
                .to_update()
                .context("unsupported assignment operator")?;

            let right = match &target {
                AssignmentTarget::Identifier(name) => {
                    self.lower_expression_with_name_hint(&assignment.right, Some(name))?
                }
                AssignmentTarget::Member { .. } | AssignmentTarget::SuperMember { .. } => {
                    self.lower_expression(&assignment.right)?
                }
            };
            let binary = match &target {
                AssignmentTarget::Identifier(name) => Expression::Binary {
                    op: lower_binary_operator(operator)?,
                    left: Box::new(Expression::Identifier(name.clone())),
                    right: Box::new(right),
                },
                AssignmentTarget::Member { object, property } => Expression::Binary {
                    op: lower_binary_operator(operator)?,
                    left: Box::new(Expression::Member {
                        object: Box::new(object.clone()),
                        property: Box::new(property.clone()),
                    }),
                    right: Box::new(right),
                },
                AssignmentTarget::SuperMember { property } => Expression::Binary {
                    op: lower_binary_operator(operator)?,
                    left: Box::new(Expression::SuperMember {
                        property: Box::new(property.clone()),
                    }),
                    right: Box::new(right),
                },
            };

            return Ok(vec![target.into_statement(binary)]);
        }

        Ok(vec![Statement::Expression(
            self.lower_expression(expression)?,
        )])
    }

    fn lower_assert_throws_statement(
        &mut self,
        call: &swc_ecma_ast::CallExpr,
    ) -> Result<Vec<Statement>> {
        ensure!(
            call.args.len() >= 2,
            "__ayyAssertThrows expects at least two arguments"
        );
        ensure!(
            call.args.iter().all(|argument| argument.spread.is_none()),
            "__ayyAssertThrows does not support spread arguments"
        );

        let callback_name = self.fresh_temporary_name("assert_throws_callback");
        let callback_value =
            self.lower_expression_with_name_hint(&call.args[1].expr, Some(&callback_name))?;
        let caught_name = self.fresh_temporary_name("assert_throws_caught");

        let mut lowered = Vec::new();
        lowered.push(Statement::Let {
            name: callback_name.clone(),
            mutable: false,
            value: callback_value,
        });
        lowered.push(Statement::Let {
            name: caught_name.clone(),
            mutable: true,
            value: Expression::Bool(false),
        });
        lowered.push(Statement::Try {
            body: vec![Statement::Expression(Expression::Call {
                callee: Box::new(Expression::Identifier(callback_name)),
                arguments: Vec::new(),
            })],
            catch_binding: None,
            catch_setup: Vec::new(),
            catch_body: vec![Statement::Assign {
                name: caught_name.clone(),
                value: Expression::Bool(true),
            }],
        });
        lowered.push(Statement::If {
            condition: Expression::Binary {
                op: BinaryOp::Equal,
                left: Box::new(Expression::Identifier(caught_name)),
                right: Box::new(Expression::Bool(false)),
            },
            then_branch: vec![Statement::Throw(Expression::Undefined)],
            else_branch: Vec::new(),
        });

        Ok(lowered)
    }

    fn lower_assignment_target(&mut self, target: &AssignTarget) -> Result<AssignmentTarget> {
        match target {
            AssignTarget::Simple(SimpleAssignTarget::Ident(identifier)) => Ok(
                AssignmentTarget::Identifier(self.resolve_binding_name(identifier.id.sym.as_ref())),
            ),
            AssignTarget::Simple(SimpleAssignTarget::Member(member)) => {
                Ok(AssignmentTarget::Member {
                    object: self.lower_expression(&member.obj)?,
                    property: self.lower_member_property(&member.prop)?,
                })
            }
            AssignTarget::Simple(SimpleAssignTarget::SuperProp(super_property)) => {
                Ok(AssignmentTarget::SuperMember {
                    property: self.lower_super_property(super_property)?,
                })
            }
            _ => bail!("unsupported assignment target"),
        }
    }

    fn lower_block_or_statement(
        &mut self,
        statement: &Stmt,
        allow_return: bool,
        allow_loop_control: bool,
    ) -> Result<Vec<Statement>> {
        match statement {
            Stmt::Block(BlockStmt { stmts, .. }) => Ok(vec![Statement::Block {
                body: self.lower_statements(stmts, allow_return, allow_loop_control)?,
            }]),
            other => self.lower_statement(other, allow_return, allow_loop_control),
        }
    }

    fn lower_optional_else(
        &mut self,
        statement: Option<&Stmt>,
        allow_return: bool,
        allow_loop_control: bool,
    ) -> Result<Vec<Statement>> {
        match statement {
            Some(statement) => {
                self.lower_block_or_statement(statement, allow_return, allow_loop_control)
            }
            None => Ok(Vec::new()),
        }
    }

    fn lower_expression(&mut self, expression: &Expr) -> Result<Expression> {
        self.lower_expression_with_name_hint(expression, None)
    }

    fn lower_expression_with_name_hint(
        &mut self,
        expression: &Expr,
        name_hint: Option<&str>,
    ) -> Result<Expression> {
        if let Some(arguments) = console_log_arguments(expression) {
            return Ok(Expression::Call {
                callee: Box::new(Expression::Identifier("__ayyPrint".to_string())),
                arguments: arguments
                    .iter()
                    .map(|argument| {
                        let expression = self.lower_expression(&argument.expr)?;
                        Ok(if argument.spread.is_some() {
                            CallArgument::Spread(expression)
                        } else {
                            CallArgument::Expression(expression)
                        })
                    })
                    .collect::<Result<Vec<_>>>()?,
            });
        }

        match expression {
            Expr::Lit(Lit::Num(number)) => Ok(Expression::Number(number.value)),
            Expr::Lit(Lit::BigInt(bigint)) => Ok(Expression::BigInt(parse_bigint_literal(
                &bigint.value.to_string(),
            )?)),
            Expr::Lit(Lit::Str(string)) => Ok(Expression::String(
                string.value.to_string_lossy().into_owned(),
            )),
            Expr::Lit(Lit::Bool(boolean)) => Ok(Expression::Bool(boolean.value)),
            Expr::Lit(Lit::Null(_)) => Ok(Expression::Null),
            Expr::MetaProp(meta_property) => match meta_property.kind {
                MetaPropKind::NewTarget => Ok(Expression::NewTarget),
                _ => bail!("unsupported expression: {expression:?}"),
            },
            Expr::Lit(Lit::Regex(regex)) => Ok(Expression::Call {
                callee: Box::new(Expression::Identifier("RegExp".to_string())),
                arguments: vec![
                    CallArgument::Expression(Expression::String(regex.exp.to_string())),
                    CallArgument::Expression(Expression::String(regex.flags.to_string())),
                ],
            }),
            Expr::Tpl(template) => self.lower_template_expression(template),
            Expr::Array(array) => Ok(Expression::Array(
                array
                    .elems
                    .iter()
                    .map(|element| match element {
                        Some(element) => {
                            let expression = self.lower_expression(&element.expr)?;
                            Ok(if element.spread.is_some() {
                                ArrayElement::Spread(expression)
                            } else {
                                ArrayElement::Expression(expression)
                            })
                        }
                        None => Ok(ArrayElement::Expression(Expression::Undefined)),
                    })
                    .collect::<Result<Vec<_>>>()?,
            )),
            Expr::Object(object) => Ok(Expression::Object(
                object
                    .props
                    .iter()
                    .map(|property| self.lower_object_entry(property))
                    .collect::<Result<Vec<_>>>()?,
            )),
            Expr::Ident(identifier) => Ok(Expression::Identifier(
                self.resolve_binding_name(identifier.sym.as_ref()),
            )),
            Expr::This(_) => Ok(Expression::This),
            Expr::Member(member) => Ok(Expression::Member {
                object: Box::new(self.lower_expression(&member.obj)?),
                property: Box::new(self.lower_member_property(&member.prop)?),
            }),
            Expr::SuperProp(super_property) => Ok(Expression::SuperMember {
                property: Box::new(self.lower_super_property(super_property)?),
            }),
            Expr::Paren(parenthesized) => {
                self.lower_expression_with_name_hint(&parenthesized.expr, name_hint)
            }
            Expr::Await(await_expression) => Ok(Expression::Await(Box::new(
                self.lower_expression_with_name_hint(&await_expression.arg, name_hint)?,
            ))),
            Expr::Unary(unary) => Ok(Expression::Unary {
                op: lower_unary_operator(unary.op)?,
                expression: Box::new(self.lower_expression(&unary.arg)?),
            }),
            Expr::Bin(binary) => Ok(Expression::Binary {
                op: lower_binary_operator(binary.op)?,
                left: Box::new(self.lower_expression(&binary.left)?),
                right: Box::new(self.lower_expression(&binary.right)?),
            }),
            Expr::Cond(conditional) => Ok(Expression::Conditional {
                condition: Box::new(self.lower_expression(&conditional.test)?),
                then_expression: Box::new(self.lower_expression(&conditional.cons)?),
                else_expression: Box::new(self.lower_expression(&conditional.alt)?),
            }),
            Expr::Seq(sequence) => Ok(Expression::Sequence(
                sequence
                    .exprs
                    .iter()
                    .map(|expression| self.lower_expression(expression))
                    .collect::<Result<Vec<_>>>()?,
            )),
            Expr::Assign(assignment) => {
                let target = self.lower_assignment_target(&assignment.left)?;
                let right = match &target {
                    AssignmentTarget::Identifier(name) => {
                        self.lower_expression_with_name_hint(&assignment.right, Some(name))?
                    }
                    AssignmentTarget::Member { .. } | AssignmentTarget::SuperMember { .. } => {
                        self.lower_expression(&assignment.right)?
                    }
                };

                match assignment.op {
                    AssignOp::Assign => self.lower_assignment_expression(target, right),
                    AssignOp::AndAssign => self.lower_logical_assignment_expression(
                        target,
                        right,
                        LogicalAssignmentKind::And,
                    ),
                    AssignOp::OrAssign => self.lower_logical_assignment_expression(
                        target,
                        right,
                        LogicalAssignmentKind::Or,
                    ),
                    AssignOp::NullishAssign => self.lower_logical_assignment_expression(
                        target,
                        right,
                        LogicalAssignmentKind::Nullish,
                    ),
                    operator => {
                        let binary_operator = lower_binary_operator(
                            operator
                                .to_update()
                                .context("unsupported assignment operator")?,
                        )?;
                        let value = match &target {
                            AssignmentTarget::Identifier(name) => Expression::Binary {
                                op: binary_operator,
                                left: Box::new(Expression::Identifier(name.clone())),
                                right: Box::new(right),
                            },
                            AssignmentTarget::Member { object, property } => Expression::Binary {
                                op: binary_operator,
                                left: Box::new(Expression::Member {
                                    object: Box::new(object.clone()),
                                    property: Box::new(property.clone()),
                                }),
                                right: Box::new(right),
                            },
                            AssignmentTarget::SuperMember { property } => Expression::Binary {
                                op: binary_operator,
                                left: Box::new(Expression::SuperMember {
                                    property: Box::new(property.clone()),
                                }),
                                right: Box::new(right),
                            },
                        };

                        self.lower_assignment_expression(target, value)
                    }
                }
            }
            Expr::Call(call) => match &call.callee {
                Callee::Expr(callee) => Ok(Expression::Call {
                    callee: Box::new(self.lower_expression(callee)?),
                    arguments: call
                        .args
                        .iter()
                        .map(|argument| {
                            let expression = self.lower_expression(&argument.expr)?;
                            Ok(if argument.spread.is_some() {
                                CallArgument::Spread(expression)
                            } else {
                                CallArgument::Expression(expression)
                            })
                        })
                        .collect::<Result<Vec<_>>>()?,
                }),
                Callee::Super(_) => {
                    let super_name = self
                        .constructor_super_stack
                        .last()
                        .and_then(|name| name.clone())
                        .context("`super()` is only supported in derived constructors")?;
                    Ok(Expression::SuperCall {
                        callee: Box::new(Expression::Identifier(super_name)),
                        arguments: call
                            .args
                            .iter()
                            .map(|argument| {
                                let expression = self.lower_expression(&argument.expr)?;
                                Ok(if argument.spread.is_some() {
                                    CallArgument::Spread(expression)
                                } else {
                                    CallArgument::Expression(expression)
                                })
                            })
                            .collect::<Result<Vec<_>>>()?,
                    })
                }
                Callee::Import(_) => self.lower_dynamic_import_expression(call),
            },
            Expr::TaggedTpl(tagged_template) => Ok(Expression::Call {
                callee: Box::new(self.lower_expression(&tagged_template.tag)?),
                arguments: std::iter::once(Ok(CallArgument::Expression(Expression::Array(
                    tagged_template
                        .tpl
                        .quasis
                        .iter()
                        .map(|quasi| {
                            Ok(ArrayElement::Expression(Expression::String(
                                quasi
                                    .cooked
                                    .as_ref()
                                    .and_then(|value| value.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                            )))
                        })
                        .collect::<Result<Vec<_>>>()?,
                ))))
                .chain(tagged_template.tpl.exprs.iter().map(|expression| {
                    self.lower_expression(expression)
                        .map(CallArgument::Expression)
                }))
                .collect::<Result<Vec<_>>>()?,
            }),
            Expr::New(new_expression) => Ok(Expression::New {
                callee: Box::new(self.lower_expression(&new_expression.callee)?),
                arguments: new_expression
                    .args
                    .as_deref()
                    .unwrap_or(&[])
                    .iter()
                    .map(|argument| {
                        let expression = self.lower_expression(&argument.expr)?;
                        Ok(if argument.spread.is_some() {
                            CallArgument::Spread(expression)
                        } else {
                            CallArgument::Expression(expression)
                        })
                    })
                    .collect::<Result<Vec<_>>>()?,
            }),
            Expr::Fn(function_expression) => {
                self.lower_function_expression(function_expression, name_hint)
            }
            Expr::Class(class_expression) => {
                self.lower_class_expression(class_expression, name_hint)
            }
            Expr::Arrow(arrow_expression) => {
                self.lower_arrow_expression(arrow_expression, name_hint)
            }
            Expr::Update(update) => {
                let name = match &*update.arg {
                    Expr::Ident(identifier) => self.resolve_binding_name(identifier.sym.as_ref()),
                    other => self
                        .try_lower_top_level_this_member_update(other)?
                        .context("only identifier update expressions are supported")?,
                };

                Ok(Expression::Update {
                    name,
                    op: lower_update_operator(update.op),
                    prefix: update.prefix,
                })
            }
            _ => bail!("unsupported expression: {expression:?}"),
        }
    }

    fn lower_assignment_expression(
        &mut self,
        target: AssignmentTarget,
        value: Expression,
    ) -> Result<Expression> {
        Ok(target.into_expression(value))
    }

    fn lower_logical_assignment_expression(
        &mut self,
        target: AssignmentTarget,
        right: Expression,
        kind: LogicalAssignmentKind,
    ) -> Result<Expression> {
        let current = target.as_expression();
        let assignment = self.lower_assignment_expression(target, right)?;

        let expression = match kind {
            LogicalAssignmentKind::And => Expression::Conditional {
                condition: Box::new(current.clone()),
                then_expression: Box::new(assignment),
                else_expression: Box::new(current),
            },
            LogicalAssignmentKind::Or => Expression::Conditional {
                condition: Box::new(current.clone()),
                then_expression: Box::new(current),
                else_expression: Box::new(assignment),
            },
            LogicalAssignmentKind::Nullish => {
                let not_undefined = Expression::Binary {
                    op: BinaryOp::NotEqual,
                    left: Box::new(current.clone()),
                    right: Box::new(Expression::Undefined),
                };
                let not_null = Expression::Binary {
                    op: BinaryOp::NotEqual,
                    left: Box::new(current.clone()),
                    right: Box::new(Expression::Null),
                };

                Expression::Conditional {
                    condition: Box::new(Expression::Binary {
                        op: BinaryOp::LogicalAnd,
                        left: Box::new(not_undefined),
                        right: Box::new(not_null),
                    }),
                    then_expression: Box::new(current),
                    else_expression: Box::new(assignment),
                }
            }
        };

        Ok(expression)
    }

    fn lower_variable_declaration(
        &mut self,
        variable_declaration: &swc_ecma_ast::VarDecl,
    ) -> Result<Vec<Statement>> {
        let mut lowered = Vec::new();

        for declarator in &variable_declaration.decls {
            if let Pat::Ident(identifier) = &declarator.name {
                let name = self.resolve_binding_name(identifier.id.sym.as_ref());

                if matches!(variable_declaration.kind, VarDeclKind::Var) {
                    let value = match declarator.init.as_deref() {
                        Some(initializer) => self.lower_expression_with_name_hint(
                            initializer,
                            Some(identifier.id.sym.as_ref()),
                        )?,
                        None => Expression::Undefined,
                    };

                    lowered.push(Statement::Var { name, value });
                } else {
                    let value = match declarator.init.as_deref() {
                        Some(initializer) => self.lower_expression_with_name_hint(
                            initializer,
                            Some(identifier.id.sym.as_ref()),
                        )?,
                        None => Expression::Undefined,
                    };

                    lowered.push(Statement::Let {
                        name,
                        mutable: !matches!(variable_declaration.kind, VarDeclKind::Const),
                        value,
                    });
                }

                continue;
            }

            if matches!(variable_declaration.kind, VarDeclKind::Var) {
                let mut names = Vec::new();
                collect_pattern_binding_names(&declarator.name, &mut names)?;
                for name in names {
                    lowered.push(Statement::Var {
                        name,
                        value: Expression::Undefined,
                    });
                }
            }

            let temporary_name = self.fresh_temporary_name("decl");
            let value = match declarator.init.as_deref() {
                Some(initializer) => self.lower_expression_with_name_hint(
                    initializer,
                    pattern_name_hint(&declarator.name),
                )?,
                None => Expression::Undefined,
            };
            lowered.push(Statement::Let {
                name: temporary_name.clone(),
                mutable: true,
                value,
            });
            self.lower_for_of_pattern_binding(
                &declarator.name,
                Expression::Identifier(temporary_name),
                if matches!(variable_declaration.kind, VarDeclKind::Var) {
                    ForOfPatternBindingKind::Assignment
                } else {
                    ForOfPatternBindingKind::Lexical {
                        mutable: !matches!(variable_declaration.kind, VarDeclKind::Const),
                    }
                },
                &mut lowered,
            )?;
        }

        Ok(lowered)
    }

    fn lower_nested_function_declaration(
        &mut self,
        function_declaration: &FnDecl,
    ) -> Result<Vec<Statement>> {
        self.next_function_expression_id += 1;
        let generated_name = format!(
            "__ayy_fnstmt_{}_{}",
            function_declaration.ident.sym, self.next_function_expression_id
        );
        let kind = lower_function_kind(
            function_declaration.function.is_generator,
            function_declaration.function.is_async,
        );
        let extra_bindings = vec![function_declaration.ident.sym.to_string()];
        let (params, body) =
            self.lower_function_parts(&function_declaration.function, &extra_bindings)?;

        self.functions.push(FunctionDeclaration {
            name: generated_name.clone(),
            top_level_binding: None,
            params,
            body,
            register_global: false,
            kind,
            self_binding: None,
            mapped_arguments: self.function_has_mapped_arguments(&function_declaration.function),
            strict: self.function_strict_mode(&function_declaration.function),
            lexical_this: false,
            length: expected_argument_count(
                function_declaration
                    .function
                    .params
                    .iter()
                    .map(|parameter| &parameter.pat),
            ),
        });

        Ok(vec![Statement::Let {
            name: self.resolve_binding_name(function_declaration.ident.sym.as_ref()),
            mutable: true,
            value: Expression::Identifier(generated_name),
        }])
    }

    fn lower_class_declaration(&mut self, class_declaration: &ClassDecl) -> Result<Vec<Statement>> {
        self.lower_class_definition(
            &class_declaration.class,
            self.resolve_binding_name(class_declaration.ident.sym.as_ref()),
        )
    }

    fn lower_class_expression(
        &mut self,
        class_expression: &swc_ecma_ast::ClassExpr,
        name_hint: Option<&str>,
    ) -> Result<Expression> {
        let class_name = class_expression
            .ident
            .as_ref()
            .map(|identifier| identifier.sym.to_string())
            .or_else(|| name_hint.map(str::to_string))
            .unwrap_or_else(|| self.fresh_temporary_name("class_expr"));
        let init_name = self.fresh_temporary_name("class_init");
        let mut init_body =
            self.lower_class_definition(&class_expression.class, class_name.clone())?;
        init_body.push(Statement::Return(Expression::Identifier(class_name)));

        self.functions.push(FunctionDeclaration {
            name: init_name.clone(),
            top_level_binding: None,
            params: Vec::new(),
            body: init_body,
            register_global: false,
            kind: FunctionKind::Ordinary,
            self_binding: None,
            mapped_arguments: false,
            strict: true,
            lexical_this: false,
            length: 0,
        });

        Ok(Expression::Call {
            callee: Box::new(Expression::Identifier(init_name)),
            arguments: Vec::new(),
        })
    }

    fn lower_class_definition(
        &mut self,
        class: &Class,
        binding_name: String,
    ) -> Result<Vec<Statement>> {
        self.private_name_scopes
            .push(self.class_private_name_map(class, &binding_name));
        let class_identifier = Expression::Identifier(binding_name.clone());
        let extends_null = matches!(class.super_class.as_deref(), Some(Expr::Lit(Lit::Null(_))));
        let super_name = class
            .super_class
            .as_ref()
            .filter(|_| !extends_null)
            .map(|_| self.fresh_temporary_name("class_super"));
        let constructor_name =
            self.lower_class_constructor(class, &binding_name, super_name.as_deref())?;
        let prototype_parent = if extends_null {
            Expression::Null
        } else {
            super_name
                .as_ref()
                .map(|name| Expression::Member {
                    object: Box::new(Expression::Identifier(name.clone())),
                    property: Box::new(Expression::String("prototype".to_string())),
                })
                .unwrap_or(Expression::Member {
                    object: Box::new(Expression::Identifier("Object".to_string())),
                    property: Box::new(Expression::String("prototype".to_string())),
                })
        };
        let prototype_target = Expression::Member {
            object: Box::new(class_identifier.clone()),
            property: Box::new(Expression::String("prototype".to_string())),
        };

        let mut statements = Vec::new();
        if let (Some(super_expression), Some(super_name)) =
            (&class.super_class, super_name.as_ref())
        {
            statements.push(Statement::Let {
                name: super_name.clone(),
                mutable: false,
                value: self.lower_expression(super_expression)?,
            });
        }

        statements.extend([
            Statement::Let {
                name: binding_name.clone(),
                mutable: true,
                value: Expression::Identifier(constructor_name),
            },
            define_property_statement(
                class_identifier.clone(),
                Expression::String("name".to_string()),
                data_property_descriptor(
                    Expression::String(binding_name.clone()),
                    false,
                    false,
                    true,
                ),
            ),
            Statement::AssignMember {
                object: class_identifier.clone(),
                property: Expression::String("prototype".to_string()),
                value: Expression::Call {
                    callee: Box::new(Expression::Member {
                        object: Box::new(Expression::Identifier("Object".to_string())),
                        property: Box::new(Expression::String("create".to_string())),
                    }),
                    arguments: vec![CallArgument::Expression(prototype_parent)],
                },
            },
            define_property_statement(
                prototype_target.clone(),
                Expression::String("constructor".to_string()),
                data_property_descriptor(class_identifier.clone(), true, false, true),
            ),
        ]);

        for member in &class.body {
            statements.extend(self.lower_class_member(member, &binding_name, &prototype_target)?);
        }

        for member in &class.body {
            if let ClassMember::PrivateProp(property) = member {
                if !property.is_static {
                    continue;
                }
                let value = property
                    .value
                    .as_ref()
                    .map(|value| self.lower_expression(value))
                    .transpose()?
                    .unwrap_or(Expression::Undefined);
                statements.push(Statement::AssignMember {
                    object: class_identifier.clone(),
                    property: self.lower_private_name(&property.key)?,
                    value,
                });
            }
        }

        self.private_name_scopes.pop();

        Ok(statements)
    }

    fn lower_class_constructor(
        &mut self,
        class: &Class,
        binding_name: &str,
        super_name: Option<&str>,
    ) -> Result<String> {
        let constructor = class.body.iter().find_map(|member| match member {
            ClassMember::Constructor(constructor) => Some(constructor),
            _ => None,
        });

        let generated_name = format!(
            "__ayy_class_ctor_{}__name_{}",
            self.fresh_temporary_name("ctor"),
            binding_name
        );

        let (params, param_setup, body, length) = if let Some(constructor) = constructor {
            let (params, param_setup, length) = lower_constructor_parameters(self, constructor)?;
            let body = if let Some(body) = &constructor.body {
                self.constructor_super_stack
                    .push(super_name.map(ToOwned::to_owned));
                self.strict_modes.push(true);
                let lowered = self.lower_statements(&body.stmts, true, false);
                self.strict_modes.pop();
                self.constructor_super_stack.pop();
                lowered?
            } else {
                Vec::new()
            };
            (params, param_setup, body, length)
        } else {
            (Vec::new(), Vec::new(), Vec::new(), 0)
        };

        let mut body = body;
        for member in class.body.iter().rev() {
            if let ClassMember::PrivateProp(property) = member {
                if property.is_static {
                    continue;
                }
                let value = property
                    .value
                    .as_ref()
                    .map(|value| self.lower_expression(value))
                    .transpose()?
                    .unwrap_or(Expression::Undefined);
                body.insert(
                    0,
                    Statement::AssignMember {
                        object: Expression::This,
                        property: self.lower_private_name(&property.key)?,
                        value,
                    },
                );
            }
        }
        body.splice(0..0, param_setup);

        self.functions.push(FunctionDeclaration {
            name: generated_name.clone(),
            top_level_binding: None,
            params,
            body,
            register_global: false,
            kind: FunctionKind::Ordinary,
            self_binding: Some(binding_name.to_string()),
            mapped_arguments: false,
            strict: true,
            lexical_this: false,
            length,
        });

        Ok(generated_name)
    }

    fn lower_class_member(
        &mut self,
        member: &ClassMember,
        class_name: &str,
        prototype_target: &Expression,
    ) -> Result<Vec<Statement>> {
        match member {
            ClassMember::Constructor(_) | ClassMember::Empty(_) | ClassMember::PrivateProp(_) => {
                Ok(Vec::new())
            }
            ClassMember::Method(method) => {
                let property = self.lower_prop_name(&method.key)?;
                let target = if method.is_static {
                    Expression::Identifier(class_name.to_string())
                } else {
                    prototype_target.clone()
                };
                if method.kind == MethodKind::Getter {
                    if let Some(private_alias) =
                        self.lower_private_method_alias_getter(method, &target)?
                    {
                        return Ok(vec![define_property_statement(
                            target,
                            property,
                            data_property_descriptor(private_alias, false, false, true),
                        )]);
                    }
                }
                self.lower_defined_class_method(
                    class_name,
                    prototype_target,
                    method.is_static,
                    method.kind,
                    property,
                    &method.function,
                )
            }
            ClassMember::PrivateMethod(method) => {
                let property = self.lower_private_name(&method.key)?;
                self.lower_defined_class_method(
                    class_name,
                    prototype_target,
                    method.is_static,
                    method.kind,
                    property,
                    &method.function,
                )
            }
            other => bail!("unsupported class member: {other:?}"),
        }
    }

    fn lower_private_method_alias_getter(
        &mut self,
        method: &ClassMethod,
        target: &Expression,
    ) -> Result<Option<Expression>> {
        let Some(body) = method.function.body.as_ref() else {
            return Ok(None);
        };
        if !method.function.params.is_empty() || body.stmts.len() != 1 {
            return Ok(None);
        }
        let swc_ecma_ast::Stmt::Return(return_statement) = &body.stmts[0] else {
            return Ok(None);
        };
        let Some(return_value) = return_statement.arg.as_deref() else {
            return Ok(None);
        };
        let Expr::Member(member) = return_value else {
            return Ok(None);
        };
        if !matches!(member.obj.as_ref(), Expr::This(_)) {
            return Ok(None);
        }
        let MemberProp::PrivateName(private_name) = &member.prop else {
            return Ok(None);
        };
        Ok(Some(Expression::Member {
            object: Box::new(target.clone()),
            property: Box::new(self.lower_private_name(private_name)?),
        }))
    }

    fn lower_defined_class_method(
        &mut self,
        class_name: &str,
        prototype_target: &Expression,
        is_static: bool,
        kind: MethodKind,
        property: Expression,
        function: &Function,
    ) -> Result<Vec<Statement>> {
        let target = if is_static {
            Expression::Identifier(class_name.to_string())
        } else {
            prototype_target.clone()
        };
        let descriptor = match kind {
            MethodKind::Method => {
                let method_name = self.lower_class_method_function(function)?;
                data_property_descriptor(Expression::Identifier(method_name), true, false, true)
            }
            MethodKind::Getter => {
                let getter_name = self.lower_class_method_function(function)?;
                getter_property_descriptor(Expression::Identifier(getter_name), false, true)
            }
            MethodKind::Setter => {
                let setter_name = self.lower_class_method_function(function)?;
                setter_property_descriptor(Expression::Identifier(setter_name), false, true)
            }
        };

        if is_static {
            return Ok(self.lower_static_class_method_definition(target, property, descriptor));
        }

        Ok(vec![define_property_statement(
            target, property, descriptor,
        )])
    }

    fn lower_static_class_method_definition(
        &mut self,
        target: Expression,
        property: Expression,
        descriptor: Expression,
    ) -> Vec<Statement> {
        let property_name = self.fresh_temporary_name("class_prop");
        let property_identifier = Expression::Identifier(property_name.clone());

        vec![
            Statement::Let {
                name: property_name,
                mutable: false,
                value: property,
            },
            Statement::If {
                condition: Expression::Binary {
                    op: BinaryOp::Equal,
                    left: Box::new(property_identifier.clone()),
                    right: Box::new(Expression::String("prototype".to_string())),
                },
                then_branch: vec![Statement::Throw(Expression::New {
                    callee: Box::new(Expression::Identifier("TypeError".to_string())),
                    arguments: Vec::new(),
                })],
                else_branch: vec![define_property_statement(
                    target,
                    property_identifier,
                    descriptor,
                )],
            },
        ]
    }

    fn lower_class_method_function(&mut self, function: &Function) -> Result<String> {
        self.next_function_expression_id += 1;
        let generated_name = format!("__ayy_class_method_{}", self.next_function_expression_id);
        self.strict_modes.push(true);
        let (params, body) = self.lower_function_parts(function, &[])?;
        self.strict_modes.pop();

        self.functions.push(FunctionDeclaration {
            name: generated_name.clone(),
            top_level_binding: None,
            params,
            body,
            register_global: false,
            kind: lower_function_kind(function.is_generator, function.is_async),
            self_binding: None,
            mapped_arguments: false,
            strict: true,
            lexical_this: false,
            length: expected_argument_count(function.params.iter().map(|parameter| &parameter.pat)),
        });

        Ok(generated_name)
    }

    fn lower_object_entry(&mut self, property: &PropOrSpread) -> Result<ObjectEntry> {
        match property {
            PropOrSpread::Spread(spread) => {
                Ok(ObjectEntry::Spread(self.lower_expression(&spread.expr)?))
            }
            PropOrSpread::Prop(property) => match &**property {
                Prop::Shorthand(identifier) => Ok(ObjectEntry::Data {
                    key: Expression::String(identifier.sym.to_string()),
                    value: Expression::Identifier(identifier.sym.to_string()),
                }),
                Prop::Method(method) => {
                    self.next_function_expression_id += 1;
                    let generated_name =
                        format!("__ayy_method_{}", self.next_function_expression_id);
                    let (params, body) = self.lower_function_parts(&method.function, &[])?;

                    self.functions.push(FunctionDeclaration {
                        name: generated_name.clone(),
                        top_level_binding: None,
                        params,
                        body,
                        register_global: false,
                        kind: lower_function_kind(
                            method.function.is_generator,
                            method.function.is_async,
                        ),
                        self_binding: None,
                        mapped_arguments: self.function_has_mapped_arguments(&method.function),
                        strict: self.function_strict_mode(&method.function),
                        lexical_this: false,
                        length: expected_argument_count(
                            method
                                .function
                                .params
                                .iter()
                                .map(|parameter| &parameter.pat),
                        ),
                    });

                    Ok(ObjectEntry::Data {
                        key: self.lower_prop_name(&method.key)?,
                        value: Expression::Identifier(generated_name),
                    })
                }
                Prop::Getter(getter) => {
                    self.next_function_expression_id += 1;
                    let generated_name =
                        format!("__ayy_getter_{}", self.next_function_expression_id);
                    let body = getter.body.as_ref().context("getters must have a body")?;
                    let strict_mode =
                        self.current_strict_mode() || script_has_use_strict_directive(&body.stmts);
                    self.strict_modes.push(strict_mode);
                    let lowered_body = self.lower_statements(&body.stmts, true, false);
                    self.strict_modes.pop();
                    let lowered_body = lowered_body?;

                    self.functions.push(FunctionDeclaration {
                        name: generated_name.clone(),
                        top_level_binding: None,
                        params: Vec::new(),
                        body: lowered_body,
                        register_global: false,
                        kind: FunctionKind::Ordinary,
                        self_binding: None,
                        mapped_arguments: false,
                        strict: strict_mode,
                        lexical_this: false,
                        length: 0,
                    });

                    Ok(ObjectEntry::Getter {
                        key: self.lower_prop_name(&getter.key)?,
                        getter: Expression::Identifier(generated_name),
                    })
                }
                Prop::Setter(setter) => {
                    self.next_function_expression_id += 1;
                    let generated_name =
                        format!("__ayy_setter_{}", self.next_function_expression_id);
                    let body = setter.body.as_ref().context("setters must have a body")?;
                    let strict_mode =
                        self.current_strict_mode() || script_has_use_strict_directive(&body.stmts);
                    self.strict_modes.push(strict_mode);
                    let lowered = (|| -> Result<(Parameter, Vec<Statement>)> {
                        let (params, mut param_setup) = lower_parameter(self, &setter.param)?;
                        let mut lowered_body = self.lower_statements(&body.stmts, true, false)?;
                        lowered_body.splice(0..0, param_setup.drain(..));
                        Ok((params, lowered_body))
                    })();
                    self.strict_modes.pop();
                    let (params, lowered_body) = lowered?;

                    self.functions.push(FunctionDeclaration {
                        name: generated_name.clone(),
                        top_level_binding: None,
                        params: vec![params],
                        body: lowered_body,
                        register_global: false,
                        kind: FunctionKind::Ordinary,
                        self_binding: None,
                        mapped_arguments: false,
                        strict: strict_mode,
                        lexical_this: false,
                        length: 1,
                    });

                    Ok(ObjectEntry::Setter {
                        key: self.lower_prop_name(&setter.key)?,
                        setter: Expression::Identifier(generated_name),
                    })
                }
                Prop::KeyValue(property) => Ok(ObjectEntry::Data {
                    key: self.lower_prop_name(&property.key)?,
                    value: self.lower_expression(&property.value)?,
                }),
                _ => {
                    bail!(
                        "only shorthand, key/value, method, getter, and setter object properties are supported"
                    )
                }
            },
        }
    }

    fn lower_template_expression(&mut self, template: &swc_ecma_ast::Tpl) -> Result<Expression> {
        let expressions = template
            .exprs
            .iter()
            .map(|expression| self.lower_expression(expression))
            .collect::<Result<Vec<_>>>()?;
        self.build_template_expression(template, &expressions)
    }

    fn lower_template_expression_with_substitution(
        &mut self,
        template: &swc_ecma_ast::Tpl,
        index: usize,
        substitution: Expression,
    ) -> Result<Expression> {
        let mut expressions = Vec::with_capacity(template.exprs.len());
        for (expression_index, expression) in template.exprs.iter().enumerate() {
            if expression_index == index {
                expressions.push(substitution.clone());
            } else {
                expressions.push(self.lower_expression(expression)?);
            }
        }
        self.build_template_expression(template, &expressions)
    }

    fn build_template_expression(
        &mut self,
        template: &swc_ecma_ast::Tpl,
        expressions: &[Expression],
    ) -> Result<Expression> {
        let mut parts = Vec::new();
        for (index, quasi) in template.quasis.iter().enumerate() {
            parts.push(Expression::String(template_quasi_text(quasi)?));
            if let Some(expression) = expressions.get(index) {
                parts.push(expression.clone());
            }
        }

        let mut expression = parts
            .into_iter()
            .reduce(|left, right| Expression::Binary {
                op: BinaryOp::Add,
                left: Box::new(left),
                right: Box::new(right),
            })
            .unwrap_or(Expression::String(String::new()));
        if !matches!(expression, Expression::String(_)) {
            expression = Expression::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expression::String(String::new())),
                right: Box::new(expression),
            };
        }
        Ok(expression)
    }

    fn lower_prop_name(&mut self, name: &PropName) -> Result<Expression> {
        Ok(match name {
            PropName::Ident(identifier) => Expression::String(identifier.sym.to_string()),
            PropName::Str(string) => {
                Expression::String(string.value.to_string_lossy().into_owned())
            }
            PropName::Num(number) => Expression::Number(number.value),
            PropName::Computed(computed) => self.lower_expression(&computed.expr)?,
            _ => bail!("unsupported object property key"),
        })
    }

    fn lower_member_property(&mut self, property: &MemberProp) -> Result<Expression> {
        Ok(match property {
            MemberProp::Ident(identifier) => Expression::String(identifier.sym.to_string()),
            MemberProp::Computed(computed) => self.lower_expression(&computed.expr)?,
            MemberProp::PrivateName(private_name) => self.lower_private_name(private_name)?,
        })
    }

    fn lower_super_property(&mut self, property: &SuperPropExpr) -> Result<Expression> {
        Ok(match &property.prop {
            SuperProp::Ident(identifier) => Expression::String(identifier.sym.to_string()),
            SuperProp::Computed(computed) => self.lower_expression(&computed.expr)?,
        })
    }

    fn try_lower_top_level_this_member_update(
        &mut self,
        expression: &Expr,
    ) -> Result<Option<String>> {
        if self.module_mode || self.strict_modes.len() != 1 {
            return Ok(None);
        }

        let Expr::Member(member) = expression else {
            return Ok(None);
        };
        if !matches!(member.obj.as_ref(), Expr::This(_)) {
            return Ok(None);
        }

        let Some(name) = static_member_property_name(&member.prop) else {
            return Ok(None);
        };
        Ok(Some(self.resolve_binding_name(&name)))
    }

    fn lower_function_parts(
        &mut self,
        function: &Function,
        extra_bindings: &[String],
    ) -> Result<(Vec<Parameter>, Vec<Statement>)> {
        let body = function
            .body
            .as_ref()
            .context("functions must have a body")?;
        let strict_mode = self.function_strict_mode(function);
        self.strict_modes.push(strict_mode);
        let mut scope_bindings = collect_parameter_binding_names(
            function.params.iter().map(|parameter| &parameter.pat),
        )?;
        for binding in collect_function_scope_binding_names(&body.stmts)? {
            if !scope_bindings.contains(&binding) {
                scope_bindings.push(binding);
            }
        }
        if !scope_bindings.iter().any(|binding| binding == "arguments") {
            scope_bindings.push("arguments".to_string());
        }
        for binding in extra_bindings {
            if !scope_bindings.contains(binding) {
                scope_bindings.push(binding.clone());
            }
        }

        self.push_binding_scope(scope_bindings);
        let lowered = (|| -> Result<(Vec<Parameter>, Vec<Statement>)> {
            let (params, param_setup) = lower_parameters(self, function)?;
            let mut body = if function.is_generator {
                self.lower_generator_statements(&body.stmts, true)?
            } else {
                self.lower_statements(&body.stmts, true, false)?
            };
            body.splice(0..0, param_setup);
            Ok((params, body))
        })();
        self.pop_binding_scope();
        self.strict_modes.pop();
        lowered.map(|(params, body)| {
            let body = if function.is_async {
                asyncify_statements(body).0
            } else {
                body
            };
            (params, body)
        })
    }
}

fn lower_parameters(
    lowerer: &mut Lowerer,
    function: &Function,
) -> Result<(Vec<Parameter>, Vec<Statement>)> {
    lower_parameter_patterns(
        lowerer,
        function.params.iter().map(|parameter| &parameter.pat),
    )
}

fn collect_parameter_binding_names<'a>(
    parameters: impl IntoIterator<Item = &'a Pat>,
) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for parameter in parameters {
        collect_pattern_binding_names(parameter, &mut names)?;
    }
    Ok(names)
}

fn collect_function_scope_binding_names(statements: &[Stmt]) -> Result<Vec<String>> {
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

fn lower_constructor_parameters(
    lowerer: &mut Lowerer,
    constructor: &Constructor,
) -> Result<(Vec<Parameter>, Vec<Statement>, usize)> {
    let mut patterns = Vec::with_capacity(constructor.params.len());
    for parameter in &constructor.params {
        let ParamOrTsParamProp::Param(parameter) = parameter else {
            bail!("parameter properties are not supported yet")
        };
        patterns.push(&parameter.pat);
    }

    let (params, setup) = lower_parameter_patterns(lowerer, patterns.iter().copied())?;
    Ok((
        params,
        setup,
        expected_argument_count(patterns.iter().copied()),
    ))
}

fn lower_parameter_patterns<'a>(
    lowerer: &mut Lowerer,
    parameters: impl IntoIterator<Item = &'a Pat>,
) -> Result<(Vec<Parameter>, Vec<Statement>)> {
    let mut lowered_parameters = Vec::new();
    let mut setup = Vec::new();

    for parameter in parameters {
        let (lowered, mut lowered_setup) = lower_parameter(lowerer, parameter)?;
        lowered_parameters.push(lowered);
        setup.append(&mut lowered_setup);
    }

    Ok((lowered_parameters, setup))
}

fn lower_parameter(lowerer: &mut Lowerer, parameter: &Pat) -> Result<(Parameter, Vec<Statement>)> {
    match parameter {
        Pat::Ident(identifier) => Ok((
            Parameter {
                name: lowerer.resolve_binding_name(identifier.id.sym.as_ref()),
                default: None,
                rest: false,
            },
            Vec::new(),
        )),
        Pat::Assign(assign) => match &*assign.left {
            Pat::Ident(identifier) => Ok((
                Parameter {
                    name: lowerer.resolve_binding_name(identifier.id.sym.as_ref()),
                    default: Some(lowerer.lower_expression(&assign.right)?),
                    rest: false,
                },
                Vec::new(),
            )),
            pattern => {
                let temporary_name = lowerer.fresh_temporary_name("param");
                let mut setup = Vec::new();
                lowerer.lower_for_of_pattern_binding(
                    pattern,
                    Expression::Identifier(temporary_name.clone()),
                    ForOfPatternBindingKind::Lexical { mutable: true },
                    &mut setup,
                )?;
                Ok((
                    Parameter {
                        name: temporary_name,
                        default: Some(lowerer.lower_expression(&assign.right)?),
                        rest: false,
                    },
                    setup,
                ))
            }
        },
        Pat::Rest(rest) => {
            if let Ok(BindingIdent { id, .. }) = binding_ident(&rest.arg) {
                return Ok((
                    Parameter {
                        name: lowerer.resolve_binding_name(id.sym.as_ref()),
                        default: None,
                        rest: true,
                    },
                    Vec::new(),
                ));
            }

            let temporary_name = lowerer.fresh_temporary_name("rest");
            let mut setup = Vec::new();
            lowerer.lower_for_of_pattern_binding(
                &rest.arg,
                Expression::Identifier(temporary_name.clone()),
                ForOfPatternBindingKind::Lexical { mutable: true },
                &mut setup,
            )?;
            Ok((
                Parameter {
                    name: temporary_name,
                    default: None,
                    rest: true,
                },
                setup,
            ))
        }
        pattern => {
            let temporary_name = lowerer.fresh_temporary_name("param");
            let mut setup = Vec::new();
            lowerer.lower_for_of_pattern_binding(
                pattern,
                Expression::Identifier(temporary_name.clone()),
                ForOfPatternBindingKind::Lexical { mutable: true },
                &mut setup,
            )?;
            Ok((
                Parameter {
                    name: temporary_name,
                    default: None,
                    rest: false,
                },
                setup,
            ))
        }
    }
}

fn expected_argument_count<'a>(parameters: impl IntoIterator<Item = &'a Pat>) -> usize {
    let mut count = 0;
    for parameter in parameters {
        match parameter {
            Pat::Rest(_) | Pat::Assign(_) => break,
            _ => count += 1,
        }
    }
    count
}

fn function_has_simple_parameter_list(function: &Function) -> bool {
    function
        .params
        .iter()
        .all(|parameter| matches!(parameter.pat, Pat::Ident(_)))
}

fn collect_for_of_binding_names(pattern: &Pat, names: &mut Vec<String>) -> Result<()> {
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

fn collect_switch_bindings(switch_statement: &SwitchStmt) -> Result<Vec<String>> {
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

fn collect_direct_statement_lexical_bindings(statements: &[Stmt]) -> Result<Vec<String>> {
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

fn collect_for_per_iteration_bindings(init: &VarDeclOrExpr) -> Result<Vec<String>> {
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

fn pattern_name_hint(pattern: &Pat) -> Option<&str> {
    match pattern {
        Pat::Ident(identifier) => Some(identifier.id.sym.as_ref()),
        _ => None,
    }
}

fn await_resume_expression() -> Expression {
    Expression::Call {
        callee: Box::new(Expression::Identifier("__ayyAwaitResume".to_string())),
        arguments: vec![CallArgument::Expression(Expression::Sent)],
    }
}

fn asyncify_statement(statement: Statement) -> (Vec<Statement>, bool) {
    match statement {
        Statement::Expression(Expression::Await(value)) => (
            vec![
                Statement::Yield { value: *value },
                Statement::Expression(await_resume_expression()),
            ],
            true,
        ),
        Statement::Var {
            name,
            value: Expression::Await(value),
        } => (
            vec![
                Statement::Yield { value: *value },
                Statement::Var {
                    name,
                    value: await_resume_expression(),
                },
            ],
            true,
        ),
        Statement::Let {
            name,
            mutable,
            value: Expression::Await(value),
        } => (
            vec![
                Statement::Yield { value: *value },
                Statement::Let {
                    name,
                    mutable,
                    value: await_resume_expression(),
                },
            ],
            true,
        ),
        Statement::Assign {
            name,
            value: Expression::Await(value),
        } => (
            vec![
                Statement::Yield { value: *value },
                Statement::Assign {
                    name,
                    value: await_resume_expression(),
                },
            ],
            true,
        ),
        Statement::Return(Expression::Await(value)) => (
            vec![
                Statement::Yield { value: *value },
                Statement::Return(await_resume_expression()),
            ],
            true,
        ),
        Statement::If {
            condition: Expression::Await(value),
            then_branch,
            else_branch,
        } => (
            vec![
                Statement::Yield { value: *value },
                Statement::If {
                    condition: await_resume_expression(),
                    then_branch,
                    else_branch,
                },
            ],
            true,
        ),
        other => (vec![other], false),
    }
}

fn asyncify_statements(statements: Vec<Statement>) -> (Vec<Statement>, bool) {
    let mut asyncified = Vec::new();
    let mut changed = false;

    for statement in statements {
        let (mut lowered, statement_changed) = asyncify_statement(statement);
        changed |= statement_changed;
        asyncified.append(&mut lowered);
    }

    (asyncified, changed)
}

fn parse_bigint_literal(value: &str) -> Result<String> {
    Ok(value.to_string())
}

fn template_quasi_text(element: &swc_ecma_ast::TplElement) -> Result<String> {
    if let Some(cooked) = &element.cooked {
        Ok(cooked.to_string_lossy().into_owned())
    } else {
        Ok(element.raw.to_string())
    }
}

fn lower_binary_operator(operator: SwcBinaryOp) -> Result<BinaryOp> {
    Ok(match operator {
        SwcBinaryOp::Add => BinaryOp::Add,
        SwcBinaryOp::Sub => BinaryOp::Subtract,
        SwcBinaryOp::Mul => BinaryOp::Multiply,
        SwcBinaryOp::Div => BinaryOp::Divide,
        SwcBinaryOp::Mod => BinaryOp::Modulo,
        SwcBinaryOp::Exp => BinaryOp::Exponentiate,
        SwcBinaryOp::BitAnd => BinaryOp::BitwiseAnd,
        SwcBinaryOp::BitOr => BinaryOp::BitwiseOr,
        SwcBinaryOp::BitXor => BinaryOp::BitwiseXor,
        SwcBinaryOp::LShift => BinaryOp::LeftShift,
        SwcBinaryOp::RShift => BinaryOp::RightShift,
        SwcBinaryOp::ZeroFillRShift => BinaryOp::UnsignedRightShift,
        SwcBinaryOp::In => BinaryOp::In,
        SwcBinaryOp::InstanceOf => BinaryOp::InstanceOf,
        SwcBinaryOp::EqEq => BinaryOp::LooseEqual,
        SwcBinaryOp::NotEq => BinaryOp::LooseNotEqual,
        SwcBinaryOp::EqEqEq => BinaryOp::Equal,
        SwcBinaryOp::NotEqEq => BinaryOp::NotEqual,
        SwcBinaryOp::Lt => BinaryOp::LessThan,
        SwcBinaryOp::LtEq => BinaryOp::LessThanOrEqual,
        SwcBinaryOp::Gt => BinaryOp::GreaterThan,
        SwcBinaryOp::GtEq => BinaryOp::GreaterThanOrEqual,
        SwcBinaryOp::LogicalAnd => BinaryOp::LogicalAnd,
        SwcBinaryOp::LogicalOr => BinaryOp::LogicalOr,
        SwcBinaryOp::NullishCoalescing => BinaryOp::NullishCoalescing,
    })
}

fn lower_unary_operator(operator: SwcUnaryOp) -> Result<UnaryOp> {
    Ok(match operator {
        SwcUnaryOp::Minus => UnaryOp::Negate,
        SwcUnaryOp::Plus => UnaryOp::Plus,
        SwcUnaryOp::Bang => UnaryOp::Not,
        SwcUnaryOp::Tilde => UnaryOp::BitwiseNot,
        SwcUnaryOp::TypeOf => UnaryOp::TypeOf,
        SwcUnaryOp::Void => UnaryOp::Void,
        SwcUnaryOp::Delete => UnaryOp::Delete,
    })
}

fn lower_update_operator(operator: SwcUpdateOp) -> UpdateOp {
    match operator {
        SwcUpdateOp::PlusPlus => UpdateOp::Increment,
        SwcUpdateOp::MinusMinus => UpdateOp::Decrement,
    }
}

fn static_member_property_name(property: &MemberProp) -> Option<String> {
    match property {
        MemberProp::Ident(identifier) => Some(identifier.sym.to_string()),
        MemberProp::Computed(computed) => match computed.expr.as_ref() {
            Expr::Lit(Lit::Str(string)) => Some(string.value.to_string_lossy().into_owned()),
            _ => None,
        },
        MemberProp::PrivateName(_) => None,
    }
}

fn lower_function_kind(is_generator: bool, is_async: bool) -> FunctionKind {
    if is_generator {
        FunctionKind::Generator
    } else if is_async {
        FunctionKind::Async
    } else {
        FunctionKind::Ordinary
    }
}

fn console_log_arguments(expression: &Expr) -> Option<&[swc_ecma_ast::ExprOrSpread]> {
    let Expr::Call(call) = expression else {
        return None;
    };

    let Callee::Expr(callee) = &call.callee else {
        return None;
    };

    let Expr::Member(member) = &**callee else {
        return None;
    };

    let Expr::Ident(object) = &*member.obj else {
        return None;
    };

    if object.sym != *"console" {
        return None;
    }

    match &member.prop {
        MemberProp::Ident(identifier) if identifier.sym == *"log" => Some(&call.args),
        _ => None,
    }
}

fn assert_throws_call(expression: &Expr) -> Option<&swc_ecma_ast::CallExpr> {
    let Expr::Call(call) = expression else {
        return None;
    };

    let Callee::Expr(callee) = &call.callee else {
        return None;
    };

    let Expr::Ident(identifier) = &**callee else {
        return None;
    };

    (identifier.sym == "__ayyAssertThrows").then_some(call)
}

fn binding_ident(pattern: &Pat) -> Result<&BindingIdent> {
    match pattern {
        Pat::Ident(identifier) => Ok(identifier),
        _ => bail!("only identifier bindings are supported"),
    }
}

enum AssignmentTarget {
    Identifier(String),
    Member {
        object: Expression,
        property: Expression,
    },
    SuperMember {
        property: Expression,
    },
}

impl AssignmentTarget {
    fn as_expression(&self) -> Expression {
        match self {
            AssignmentTarget::Identifier(name) => Expression::Identifier(name.clone()),
            AssignmentTarget::Member { object, property } => Expression::Member {
                object: Box::new(object.clone()),
                property: Box::new(property.clone()),
            },
            AssignmentTarget::SuperMember { property } => Expression::SuperMember {
                property: Box::new(property.clone()),
            },
        }
    }

    fn into_statement(self, value: Expression) -> Statement {
        match self {
            AssignmentTarget::Identifier(name) => Statement::Assign { name, value },
            AssignmentTarget::Member { object, property } => Statement::AssignMember {
                object,
                property,
                value,
            },
            AssignmentTarget::SuperMember { property } => {
                Statement::Expression(Expression::AssignSuperMember {
                    property: Box::new(property),
                    value: Box::new(value),
                })
            }
        }
    }

    fn into_expression(self, value: Expression) -> Expression {
        match self {
            AssignmentTarget::Identifier(name) => Expression::Assign {
                name,
                value: Box::new(value),
            },
            AssignmentTarget::Member { object, property } => Expression::AssignMember {
                object: Box::new(object),
                property: Box::new(property),
                value: Box::new(value),
            },
            AssignmentTarget::SuperMember { property } => Expression::AssignSuperMember {
                property: Box::new(property),
                value: Box::new(value),
            },
        }
    }
}

struct ForOfBinding {
    before_loop: Vec<Statement>,
    per_iteration: Vec<Statement>,
}

#[derive(Clone, Copy)]
enum ForOfPatternBindingKind {
    Assignment,
    Var,
    Lexical { mutable: bool },
}

#[derive(Clone, Copy)]
enum LogicalAssignmentKind {
    And,
    Or,
    Nullish,
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn parses_hashbang_comments_terminated_by_carriage_return() {
        parse("#! comment\r{}\n").expect("carriage-return-terminated hashbang should parse");
    }

    #[test]
    fn parses_hashbang_comments_terminated_by_line_separator() {
        parse("#! comment\u{2028}{}\n").expect("line-separator-terminated hashbang should parse");
    }

    #[test]
    fn parses_hashbang_comments_terminated_by_paragraph_separator() {
        parse("#! comment\u{2029}{}\n")
            .expect("paragraph-separator-terminated hashbang should parse");
    }
}
