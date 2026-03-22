use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, bail};

use crate::hir::{
    ArrayElement, CallArgument, Expression, FunctionDeclaration, ObjectEntry, Program, Statement,
    SwitchCase,
};

pub fn validate_refined_aot(program: &Program) -> Result<()> {
    RefinedAotValidator::new(program).validate()
}

pub fn lower_static_function_constructors(program: Program) -> Result<Program> {
    StaticFunctionConstructorLowerer::new(&program).lower(program)
}

struct StaticFunctionConstructorLowerer {
    scopes: Vec<HashSet<String>>,
    global_scope: HashSet<String>,
    existing_function_names: HashSet<String>,
    synthetic_functions: Vec<FunctionDeclaration>,
    next_synthetic_function_id: usize,
}

impl StaticFunctionConstructorLowerer {
    fn new(program: &Program) -> Self {
        let mut global_scope = collect_statement_bindings(program.statements.iter())
            .into_iter()
            .collect::<HashSet<_>>();
        global_scope.extend(
            program
                .functions
                .iter()
                .filter(|function| function.register_global)
                .map(|function| function.name.clone()),
        );

        Self {
            scopes: Vec::new(),
            global_scope,
            existing_function_names: program
                .functions
                .iter()
                .map(|function| function.name.clone())
                .collect(),
            synthetic_functions: Vec::new(),
            next_synthetic_function_id: 0,
        }
    }

    fn lower(mut self, mut program: Program) -> Result<Program> {
        self.scopes.push(self.global_scope.clone());
        program.statements = self.lower_statement_list(program.statements)?;

        let original_functions = std::mem::take(&mut program.functions);
        let mut lowered_functions = Vec::with_capacity(original_functions.len());
        for function in original_functions {
            lowered_functions.push(self.lower_function(function)?);
        }
        self.scopes.pop();

        lowered_functions.extend(self.synthetic_functions);
        program.functions = lowered_functions;
        Ok(program)
    }

    fn lower_function(&mut self, mut function: FunctionDeclaration) -> Result<FunctionDeclaration> {
        let mut function_scope = collect_statement_bindings(function.body.iter())
            .into_iter()
            .collect::<HashSet<_>>();
        function_scope.extend(
            function
                .params
                .iter()
                .map(|parameter| parameter.name.clone()),
        );
        if let Some(self_binding) = &function.self_binding {
            function_scope.insert(self_binding.clone());
        }
        function_scope.insert("arguments".to_string());

        self.scopes.push(function_scope);
        for parameter in &mut function.params {
            if let Some(default) = parameter.default.take() {
                parameter.default = Some(self.lower_expression(default)?);
            }
        }
        function.body = self.lower_statement_list(function.body)?;
        self.scopes.pop();
        Ok(function)
    }

    fn lower_synthetic_function(
        &mut self,
        mut function: FunctionDeclaration,
    ) -> Result<FunctionDeclaration> {
        function.top_level_binding = None;
        function.register_global = false;
        function.self_binding = None;

        let saved_scopes = std::mem::take(&mut self.scopes);
        self.scopes.push(self.global_scope.clone());
        let result = self.lower_function(function);
        self.scopes = saved_scopes;
        result
    }

    fn lower_statement_list(&mut self, statements: Vec<Statement>) -> Result<Vec<Statement>> {
        statements
            .into_iter()
            .map(|statement| self.lower_statement(statement))
            .collect()
    }

    fn lower_scoped_statement_list(
        &mut self,
        statements: Vec<Statement>,
        extra_bindings: impl IntoIterator<Item = String>,
    ) -> Result<Vec<Statement>> {
        let mut scope = collect_statement_bindings(statements.iter())
            .into_iter()
            .collect::<HashSet<_>>();
        scope.extend(extra_bindings);
        self.scopes.push(scope);
        let result = self.lower_statement_list(statements);
        self.scopes.pop();
        result
    }

    fn lower_statement(&mut self, statement: Statement) -> Result<Statement> {
        match statement {
            Statement::Block { body } => Ok(Statement::Block {
                body: self.lower_scoped_statement_list(body, [])?,
            }),
            Statement::Labeled { labels, body } => Ok(Statement::Labeled {
                labels,
                body: self.lower_scoped_statement_list(body, [])?,
            }),
            Statement::Var { name, value } => Ok(Statement::Var {
                name,
                value: self.lower_expression(value)?,
            }),
            Statement::Let {
                name,
                mutable,
                value,
            } => Ok(Statement::Let {
                name,
                mutable,
                value: self.lower_expression(value)?,
            }),
            Statement::Assign { name, value } => Ok(Statement::Assign {
                name,
                value: self.lower_expression(value)?,
            }),
            Statement::AssignMember {
                object,
                property,
                value,
            } => Ok(Statement::AssignMember {
                object: self.lower_expression(object)?,
                property: self.lower_expression(property)?,
                value: self.lower_expression(value)?,
            }),
            Statement::Print { values } => Ok(Statement::Print {
                values: values
                    .into_iter()
                    .map(|value| self.lower_expression(value))
                    .collect::<Result<Vec<_>>>()?,
            }),
            Statement::Expression(value) => {
                Ok(Statement::Expression(self.lower_expression(value)?))
            }
            Statement::Throw(value) => Ok(Statement::Throw(self.lower_expression(value)?)),
            Statement::Return(value) => Ok(Statement::Return(self.lower_expression(value)?)),
            Statement::Break { label } => Ok(Statement::Break { label }),
            Statement::Continue { label } => Ok(Statement::Continue { label }),
            Statement::Yield { value } => Ok(Statement::Yield {
                value: self.lower_expression(value)?,
            }),
            Statement::YieldDelegate { value } => Ok(Statement::YieldDelegate {
                value: self.lower_expression(value)?,
            }),
            Statement::With { object, body } => Ok(Statement::With {
                object: self.lower_expression(object)?,
                body: self.lower_scoped_statement_list(body, [])?,
            }),
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => Ok(Statement::If {
                condition: self.lower_expression(condition)?,
                then_branch: self.lower_scoped_statement_list(then_branch, [])?,
                else_branch: self.lower_scoped_statement_list(else_branch, [])?,
            }),
            Statement::Try {
                body,
                catch_binding,
                catch_setup,
                catch_body,
            } => {
                let body = self.lower_scoped_statement_list(body, [])?;

                let mut catch_bindings =
                    collect_statement_bindings(catch_setup.iter().chain(catch_body.iter()));
                if let Some(binding) = &catch_binding {
                    catch_bindings.push(binding.clone());
                }

                let catch_setup =
                    self.lower_scoped_statement_list(catch_setup, catch_bindings.iter().cloned())?;
                let catch_body = self.lower_scoped_statement_list(catch_body, catch_bindings)?;

                Ok(Statement::Try {
                    body,
                    catch_binding,
                    catch_setup,
                    catch_body,
                })
            }
            Statement::Switch {
                labels,
                bindings,
                discriminant,
                cases,
            } => {
                self.scopes
                    .push(bindings.iter().cloned().collect::<HashSet<_>>());
                let result = (|| -> Result<Vec<SwitchCase>> {
                    cases
                        .into_iter()
                        .map(|case| {
                            Ok(SwitchCase {
                                test: match case.test {
                                    Some(test) => Some(self.lower_expression(test)?),
                                    None => None,
                                },
                                body: self.lower_statement_list(case.body)?,
                            })
                        })
                        .collect()
                })();
                self.scopes.pop();

                Ok(Statement::Switch {
                    labels,
                    bindings,
                    discriminant: self.lower_expression(discriminant)?,
                    cases: result?,
                })
            }
            Statement::For {
                labels,
                init,
                per_iteration_bindings,
                condition,
                update,
                break_hook,
                body,
            } => {
                let mut loop_bindings = collect_statement_bindings(init.iter());
                loop_bindings.extend(per_iteration_bindings.iter().cloned());

                self.scopes
                    .push(loop_bindings.into_iter().collect::<HashSet<_>>());
                let result = (|| -> Result<_> {
                    Ok(Statement::For {
                        labels,
                        init: self.lower_statement_list(init)?,
                        per_iteration_bindings,
                        condition: match condition {
                            Some(condition) => Some(self.lower_expression(condition)?),
                            None => None,
                        },
                        update: match update {
                            Some(update) => Some(self.lower_expression(update)?),
                            None => None,
                        },
                        break_hook: match break_hook {
                            Some(break_hook) => Some(self.lower_expression(break_hook)?),
                            None => None,
                        },
                        body: self.lower_statement_list(body)?,
                    })
                })();
                self.scopes.pop();
                result
            }
            Statement::While {
                labels,
                condition,
                break_hook,
                body,
            } => Ok(Statement::While {
                labels,
                condition: self.lower_expression(condition)?,
                break_hook: match break_hook {
                    Some(break_hook) => Some(self.lower_expression(break_hook)?),
                    None => None,
                },
                body: self.lower_scoped_statement_list(body, [])?,
            }),
            Statement::DoWhile {
                labels,
                condition,
                break_hook,
                body,
            } => Ok(Statement::DoWhile {
                labels,
                condition: self.lower_expression(condition)?,
                break_hook: match break_hook {
                    Some(break_hook) => Some(self.lower_expression(break_hook)?),
                    None => None,
                },
                body: self.lower_scoped_statement_list(body, [])?,
            }),
        }
    }

    fn lower_expression(&mut self, expression: Expression) -> Result<Expression> {
        match expression {
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::NewTarget
            | Expression::Identifier(_)
            | Expression::This
            | Expression::Sent => Ok(expression),
            Expression::Array(elements) => Ok(Expression::Array(
                elements
                    .into_iter()
                    .map(|element| match element {
                        ArrayElement::Expression(expression) => {
                            Ok(ArrayElement::Expression(self.lower_expression(expression)?))
                        }
                        ArrayElement::Spread(expression) => {
                            Ok(ArrayElement::Spread(self.lower_expression(expression)?))
                        }
                    })
                    .collect::<Result<Vec<_>>>()?,
            )),
            Expression::Object(entries) => Ok(Expression::Object(
                entries
                    .into_iter()
                    .map(|entry| match entry {
                        ObjectEntry::Data { key, value } => Ok(ObjectEntry::Data {
                            key: self.lower_expression(key)?,
                            value: self.lower_expression(value)?,
                        }),
                        ObjectEntry::Getter { key, getter } => Ok(ObjectEntry::Getter {
                            key: self.lower_expression(key)?,
                            getter: self.lower_expression(getter)?,
                        }),
                        ObjectEntry::Setter { key, setter } => Ok(ObjectEntry::Setter {
                            key: self.lower_expression(key)?,
                            setter: self.lower_expression(setter)?,
                        }),
                        ObjectEntry::Spread(expression) => {
                            Ok(ObjectEntry::Spread(self.lower_expression(expression)?))
                        }
                    })
                    .collect::<Result<Vec<_>>>()?,
            )),
            Expression::Member { object, property } => Ok(Expression::Member {
                object: Box::new(self.lower_expression(*object)?),
                property: Box::new(self.lower_expression(*property)?),
            }),
            Expression::SuperMember { property } => Ok(Expression::SuperMember {
                property: Box::new(self.lower_expression(*property)?),
            }),
            Expression::Assign { name, value } => Ok(Expression::Assign {
                name,
                value: Box::new(self.lower_expression(*value)?),
            }),
            Expression::AssignMember {
                object,
                property,
                value,
            } => Ok(Expression::AssignMember {
                object: Box::new(self.lower_expression(*object)?),
                property: Box::new(self.lower_expression(*property)?),
                value: Box::new(self.lower_expression(*value)?),
            }),
            Expression::AssignSuperMember { property, value } => {
                Ok(Expression::AssignSuperMember {
                    property: Box::new(self.lower_expression(*property)?),
                    value: Box::new(self.lower_expression(*value)?),
                })
            }
            Expression::Await(expression) => Ok(Expression::Await(Box::new(
                self.lower_expression(*expression)?,
            ))),
            Expression::EnumerateKeys(expression) => Ok(Expression::EnumerateKeys(Box::new(
                self.lower_expression(*expression)?,
            ))),
            Expression::GetIterator(expression) => Ok(Expression::GetIterator(Box::new(
                self.lower_expression(*expression)?,
            ))),
            Expression::IteratorClose(expression) => Ok(Expression::IteratorClose(Box::new(
                self.lower_expression(*expression)?,
            ))),
            Expression::Unary { op, expression } => Ok(Expression::Unary {
                op,
                expression: Box::new(self.lower_expression(*expression)?),
            }),
            Expression::Binary { op, left, right } => Ok(Expression::Binary {
                op,
                left: Box::new(self.lower_expression(*left)?),
                right: Box::new(self.lower_expression(*right)?),
            }),
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => Ok(Expression::Conditional {
                condition: Box::new(self.lower_expression(*condition)?),
                then_expression: Box::new(self.lower_expression(*then_expression)?),
                else_expression: Box::new(self.lower_expression(*else_expression)?),
            }),
            Expression::Sequence(expressions) => Ok(Expression::Sequence(
                expressions
                    .into_iter()
                    .map(|expression| self.lower_expression(expression))
                    .collect::<Result<Vec<_>>>()?,
            )),
            Expression::Call { callee, arguments } => {
                let callee = self.lower_expression(*callee)?;
                let arguments = self.lower_arguments(arguments)?;
                if let Some(lowered) =
                    self.try_lower_static_function_constructor(&callee, &arguments)?
                {
                    return Ok(lowered);
                }
                Ok(Expression::Call {
                    callee: Box::new(callee),
                    arguments,
                })
            }
            Expression::SuperCall { callee, arguments } => Ok(Expression::SuperCall {
                callee: Box::new(self.lower_expression(*callee)?),
                arguments: self.lower_arguments(arguments)?,
            }),
            Expression::New { callee, arguments } => {
                let callee = self.lower_expression(*callee)?;
                let arguments = self.lower_arguments(arguments)?;
                if let Some(lowered) =
                    self.try_lower_static_function_constructor(&callee, &arguments)?
                {
                    return Ok(lowered);
                }
                Ok(Expression::New {
                    callee: Box::new(callee),
                    arguments,
                })
            }
            Expression::Update { .. } => Ok(expression),
        }
    }

    fn lower_arguments(&mut self, arguments: Vec<CallArgument>) -> Result<Vec<CallArgument>> {
        arguments
            .into_iter()
            .map(|argument| match argument {
                CallArgument::Expression(expression) => {
                    Ok(CallArgument::Expression(self.lower_expression(expression)?))
                }
                CallArgument::Spread(expression) => {
                    Ok(CallArgument::Spread(self.lower_expression(expression)?))
                }
            })
            .collect()
    }

    fn try_lower_static_function_constructor(
        &mut self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Result<Option<Expression>> {
        if !self.is_function_constructor_callee(callee) {
            return Ok(None);
        }

        let Some((parameter_source, body_source)) =
            function_constructor_literal_source_parts(arguments)
        else {
            return Ok(None);
        };

        let function_name = self.fresh_function_name();
        let wrapper_source =
            format!("function {function_name}({parameter_source}) {{\n{body_source}\n}}");
        let parsed = crate::frontend::parse(&wrapper_source).with_context(|| {
            format!("failed to parse static Function constructor source for `{function_name}`")
        })?;
        let Some(function) = parsed
            .functions
            .into_iter()
            .find(|function| function.name == function_name)
        else {
            bail!("failed to lower static Function constructor `{function_name}`");
        };

        let lowered_function = self.lower_synthetic_function(function)?;
        self.synthetic_functions.push(lowered_function);
        Ok(Some(Expression::Identifier(function_name)))
    }

    fn fresh_function_name(&mut self) -> String {
        loop {
            let candidate = format!("__ayy_function_ctor_{}", self.next_synthetic_function_id);
            self.next_synthetic_function_id += 1;
            if self.existing_function_names.insert(candidate.clone()) {
                return candidate;
            }
        }
    }

    fn is_bound(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|scope| scope.contains(name))
    }

    fn is_global_identifier(&self, expression: &Expression, name: &str) -> bool {
        matches!(expression, Expression::Identifier(identifier) if identifier == name && !self.is_bound(identifier))
    }

    fn is_string_literal(&self, expression: &Expression, value: &str) -> bool {
        matches!(expression, Expression::String(string) if string == value)
    }

    fn is_function_constructor_callee(&self, callee: &Expression) -> bool {
        self.is_global_identifier(callee, "Function")
            || matches!(
                callee,
                Expression::Member { object, property }
                    if self.is_global_identifier(object, "globalThis")
                        && self.is_string_literal(property, "Function")
            )
    }
}

struct RefinedAotValidator<'a> {
    program: &'a Program,
    functions: HashMap<&'a str, &'a FunctionDeclaration>,
    validated_functions: HashSet<&'a str>,
    scopes: Vec<HashSet<String>>,
    known_kinds: Vec<HashMap<String, KnownValueKind>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum KnownValueKind {
    String,
    NonString,
    Unknown,
}

impl<'a> RefinedAotValidator<'a> {
    fn new(program: &'a Program) -> Self {
        Self {
            program,
            functions: program
                .functions
                .iter()
                .map(|function| (function.name.as_str(), function))
                .collect(),
            validated_functions: HashSet::new(),
            scopes: Vec::new(),
            known_kinds: Vec::new(),
        }
    }

    fn validate(mut self) -> Result<()> {
        let mut global_scope = collect_statement_bindings(self.program.statements.iter())
            .into_iter()
            .collect::<HashSet<_>>();
        global_scope.extend(
            self.program
                .functions
                .iter()
                .filter(|function| function.register_global)
                .map(|function| function.name.clone()),
        );

        self.scopes.push(global_scope);
        self.known_kinds.push(HashMap::new());
        self.validate_statement_list(&self.program.statements)?;
        for function in self
            .program
            .functions
            .iter()
            .filter(|function| function.register_global)
        {
            self.validate_function(function)?;
        }
        self.scopes.pop();
        self.known_kinds.pop();

        Ok(())
    }

    fn validate_function(&mut self, function: &'a FunctionDeclaration) -> Result<()> {
        if !self.validated_functions.insert(function.name.as_str()) {
            return Ok(());
        }

        let mut function_scope = collect_statement_bindings(function.body.iter())
            .into_iter()
            .collect::<HashSet<_>>();
        function_scope.extend(
            function
                .params
                .iter()
                .map(|parameter| parameter.name.clone()),
        );
        if let Some(self_binding) = &function.self_binding {
            function_scope.insert(self_binding.clone());
        }
        function_scope.insert("arguments".to_string());

        self.scopes.push(function_scope);
        self.known_kinds.push(HashMap::new());

        for parameter in &function.params {
            if let Some(default) = &parameter.default {
                self.validate_expression(default)?;
            }
        }
        self.validate_statement_list(&function.body)?;

        self.scopes.pop();
        self.known_kinds.pop();
        Ok(())
    }

    fn validate_statement_list(&mut self, statements: &[Statement]) -> Result<()> {
        for statement in statements {
            self.validate_statement(statement)?;
        }
        Ok(())
    }

    fn validate_scoped_statement_list(
        &mut self,
        statements: &[Statement],
        extra_bindings: impl IntoIterator<Item = String>,
    ) -> Result<()> {
        let mut scope = collect_statement_bindings(statements.iter())
            .into_iter()
            .collect::<HashSet<_>>();
        scope.extend(extra_bindings);
        self.scopes.push(scope);
        self.known_kinds.push(HashMap::new());
        let result = self.validate_statement_list(statements);
        self.known_kinds.pop();
        self.scopes.pop();
        result
    }

    fn validate_statement(&mut self, statement: &Statement) -> Result<()> {
        match statement {
            Statement::Block { body } | Statement::Labeled { body, .. } => {
                self.validate_scoped_statement_list(body, [])
            }
            Statement::Var { name, value } => {
                self.validate_expression(value)?;
                let kind = self.infer_known_kind(value);
                self.record_known_kind(name, kind);
                Ok(())
            }
            Statement::Let { name, value, .. } => {
                self.validate_expression(value)?;
                let kind = self.infer_known_kind(value);
                self.record_known_kind(name, kind);
                Ok(())
            }
            Statement::Assign { name, value } => {
                self.validate_expression(value)?;
                let kind = self.infer_known_kind(value);
                self.record_known_kind(name, kind);
                Ok(())
            }
            Statement::Expression(value)
            | Statement::Throw(value)
            | Statement::Return(value)
            | Statement::Yield { value }
            | Statement::YieldDelegate { value } => self.validate_expression(value),
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                self.validate_expression(object)?;
                self.validate_expression(property)?;
                self.validate_expression(value)
            }
            Statement::Print { values } => {
                for value in values {
                    self.validate_expression(value)?;
                }
                Ok(())
            }
            Statement::With { object, body } => {
                self.validate_expression(object)?;
                self.validate_scoped_statement_list(body, [])
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.validate_expression(condition)?;
                self.validate_scoped_statement_list(then_branch, [])?;
                self.validate_scoped_statement_list(else_branch, [])
            }
            Statement::Try {
                body,
                catch_binding,
                catch_setup,
                catch_body,
            } => {
                self.validate_scoped_statement_list(body, [])?;

                let mut catch_bindings =
                    collect_statement_bindings(catch_setup.iter().chain(catch_body.iter()));
                if let Some(catch_binding) = catch_binding {
                    catch_bindings.push(catch_binding.clone());
                }

                self.validate_scoped_statement_list(catch_setup, catch_bindings.iter().cloned())?;
                self.validate_scoped_statement_list(catch_body, catch_bindings)
            }
            Statement::Switch {
                bindings,
                discriminant,
                cases,
                ..
            } => {
                self.validate_expression(discriminant)?;

                self.scopes
                    .push(bindings.iter().cloned().collect::<HashSet<_>>());
                let result = (|| -> Result<()> {
                    for case in cases {
                        if let Some(test) = &case.test {
                            self.validate_expression(test)?;
                        }
                        self.validate_statement_list(&case.body)?;
                    }
                    Ok(())
                })();
                self.scopes.pop();
                result
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
                let mut loop_bindings = collect_statement_bindings(init.iter());
                loop_bindings.extend(per_iteration_bindings.iter().cloned());

                self.scopes
                    .push(loop_bindings.into_iter().collect::<HashSet<_>>());
                let result = (|| -> Result<()> {
                    for statement in init {
                        self.validate_statement(statement)?;
                    }
                    if let Some(condition) = condition {
                        self.validate_expression(condition)?;
                    }
                    if let Some(update) = update {
                        self.validate_expression(update)?;
                    }
                    if let Some(break_hook) = break_hook {
                        self.validate_expression(break_hook)?;
                    }
                    self.validate_statement_list(body)
                })();
                self.scopes.pop();
                result
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
                self.validate_expression(condition)?;
                if let Some(break_hook) = break_hook {
                    self.validate_expression(break_hook)?;
                }
                self.validate_scoped_statement_list(body, [])
            }
            Statement::Break { .. } | Statement::Continue { .. } => Ok(()),
        }
    }

    fn validate_expression(&mut self, expression: &Expression) -> Result<()> {
        match expression {
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::NewTarget
            | Expression::This
            | Expression::Sent => Ok(()),
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                            self.validate_expression(expression)?
                        }
                    }
                }
                Ok(())
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        ObjectEntry::Data { key, value } => {
                            self.validate_expression(key)?;
                            self.validate_expression(value)?;
                        }
                        ObjectEntry::Getter { key, getter }
                        | ObjectEntry::Setter {
                            key,
                            setter: getter,
                        } => {
                            self.validate_expression(key)?;
                            self.validate_expression(getter)?;
                        }
                        ObjectEntry::Spread(expression) => {
                            self.validate_expression(expression)?;
                        }
                    }
                }
                Ok(())
            }
            Expression::Identifier(name) => {
                if let Some(function) = self.functions.get(name.as_str()).copied() {
                    self.validate_function(function)?;
                }

                Ok(())
            }
            Expression::Member { object, property } => {
                self.validate_expression(object)?;
                self.validate_expression(property)?;

                if self.is_global_identifier(object, "Realm")
                    && self.is_string_literal(property, "eval")
                {
                    bail!("refined AOT goal forbids runtime source evaluation via `Realm.eval`");
                }
                if self.is_global_identifier(object, "$262")
                    && self.is_string_literal(property, "evalScript")
                {
                    bail!(
                        "refined AOT goal forbids runtime source evaluation via `$262.evalScript`"
                    );
                }
                if self.is_global_identifier(object, "globalThis")
                    && self.is_string_literal(property, "eval")
                {
                    bail!(
                        "refined AOT goal forbids runtime source evaluation via `globalThis.eval`"
                    );
                }

                Ok(())
            }
            Expression::SuperMember { property } => self.validate_expression(property),
            Expression::Assign { value, .. } => self.validate_expression(value),
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.validate_expression(object)?;
                self.validate_expression(property)?;
                self.validate_expression(value)
            }
            Expression::AssignSuperMember { property, value } => {
                self.validate_expression(property)?;
                self.validate_expression(value)
            }
            Expression::Await(expression)
            | Expression::EnumerateKeys(expression)
            | Expression::GetIterator(expression)
            | Expression::IteratorClose(expression)
            | Expression::Unary { expression, .. } => self.validate_expression(expression),
            Expression::Binary { left, right, .. } => {
                self.validate_expression(left)?;
                self.validate_expression(right)
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.validate_expression(condition)?;
                self.validate_expression(then_expression)?;
                self.validate_expression(else_expression)
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.validate_expression(expression)?;
                }
                Ok(())
            }
            Expression::Call { callee, arguments } => {
                self.validate_expression(callee)?;
                self.validate_arguments(arguments)?;

                if self.is_direct_literal_eval_call(callee, arguments)
                    || self.is_direct_comment_eval_call(callee, arguments)
                    || self.is_direct_non_string_eval_call(callee, arguments)
                {
                    return Ok(());
                }

                if self.is_global_identifier(callee, "eval") {
                    bail!(
                        "refined AOT goal currently permits builtin `eval` only when called directly with a compile-time string literal"
                    );
                }

                if (self.is_function_constructor_callee(callee)
                    && !function_constructor_literal_source_parts(arguments).is_some())
                    || self.is_reflect_construct_function(callee, arguments)
                {
                    bail!(
                        "refined AOT goal forbids runtime source evaluation via the `Function` constructor"
                    );
                }

                Ok(())
            }
            Expression::New { callee, arguments } => {
                self.validate_expression(callee)?;
                self.validate_arguments(arguments)?;

                if self.is_function_constructor_callee(callee)
                    && !function_constructor_literal_source_parts(arguments).is_some()
                    || self.is_reflect_construct_function(callee, arguments)
                {
                    bail!(
                        "refined AOT goal forbids runtime source evaluation via the `Function` constructor"
                    );
                }

                Ok(())
            }
            Expression::SuperCall { callee, arguments } => {
                self.validate_expression(callee)?;
                self.validate_arguments(arguments)
            }
            Expression::Update { .. } => Ok(()),
        }
    }

    fn validate_arguments(&mut self, arguments: &[CallArgument]) -> Result<()> {
        for argument in arguments {
            match argument {
                CallArgument::Expression(argument) | CallArgument::Spread(argument) => {
                    self.validate_expression(argument)?
                }
            }
        }
        Ok(())
    }

    fn is_bound(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|scope| scope.contains(name))
    }

    fn record_known_kind(&mut self, name: &str, kind: KnownValueKind) {
        if let Some(scope) = self.known_kinds.last_mut() {
            scope.insert(name.to_string(), kind);
        }
    }

    fn lookup_known_kind(&self, name: &str) -> KnownValueKind {
        if let Some(kind) = self
            .known_kinds
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
        {
            return kind;
        }
        if self.functions.contains_key(name) {
            return KnownValueKind::NonString;
        }
        if matches!(name, "undefined" | "NaN" | "Infinity") {
            return KnownValueKind::NonString;
        }
        KnownValueKind::Unknown
    }

    fn infer_known_kind(&self, expression: &Expression) -> KnownValueKind {
        match expression {
            Expression::String(_) => KnownValueKind::String,
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::Array(_)
            | Expression::Object(_)
            | Expression::New { .. }
            | Expression::This
            | Expression::NewTarget
            | Expression::Sent => KnownValueKind::NonString,
            Expression::Identifier(name) => self.lookup_known_kind(name),
            Expression::Unary { op, .. } => match op {
                crate::hir::UnaryOp::TypeOf => KnownValueKind::String,
                crate::hir::UnaryOp::Not | crate::hir::UnaryOp::Delete => KnownValueKind::NonString,
                crate::hir::UnaryOp::Void
                | crate::hir::UnaryOp::Plus
                | crate::hir::UnaryOp::Negate
                | crate::hir::UnaryOp::BitwiseNot => KnownValueKind::NonString,
            },
            Expression::Assign { value, .. }
            | Expression::AssignMember { value, .. }
            | Expression::AssignSuperMember { value, .. } => self.infer_known_kind(value),
            _ => KnownValueKind::Unknown,
        }
    }

    fn is_global_identifier(&self, expression: &Expression, name: &str) -> bool {
        matches!(expression, Expression::Identifier(identifier) if identifier == name && !self.is_bound(identifier))
    }

    fn is_string_literal(&self, expression: &Expression, value: &str) -> bool {
        matches!(expression, Expression::String(string) if string == value)
    }

    fn is_function_constructor_callee(&self, callee: &Expression) -> bool {
        self.is_global_identifier(callee, "Function")
            || matches!(
                callee,
                Expression::Member { object, property }
                    if self.is_global_identifier(object, "globalThis")
                        && self.is_string_literal(property, "Function")
            )
    }

    fn is_direct_literal_eval_call(&self, callee: &Expression, arguments: &[CallArgument]) -> bool {
        if !self.is_global_identifier(callee, "eval") {
            return false;
        }

        match arguments.first() {
            None => true,
            Some(CallArgument::Expression(Expression::String(_))) => true,
            _ => false,
        }
    }

    fn is_direct_non_string_eval_call(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> bool {
        if !self.is_global_identifier(callee, "eval") {
            return false;
        }

        match arguments.first() {
            Some(CallArgument::Expression(argument)) => {
                self.infer_known_kind(argument) == KnownValueKind::NonString
            }
            _ => false,
        }
    }

    fn is_direct_comment_eval_call(&self, callee: &Expression, arguments: &[CallArgument]) -> bool {
        if !self.is_global_identifier(callee, "eval") {
            return false;
        }

        let Some(CallArgument::Expression(argument)) = arguments.first() else {
            return false;
        };

        let mut fragments = Vec::new();
        if !self.collect_string_concat_fragments(argument, &mut fragments) {
            return false;
        }

        matches!(
            fragments.as_slice(),
            [
                EvalStringFragment::Static(prefix),
                EvalStringFragment::Dynamic,
                EvalStringFragment::Static(suffix),
            ] if (prefix == "//var " && suffix == "yy = -1")
                || (prefix == "/*var " && suffix == "xx = 1*/")
        )
    }

    fn collect_string_concat_fragments(
        &self,
        expression: &Expression,
        fragments: &mut Vec<EvalStringFragment>,
    ) -> bool {
        if let Expression::Binary {
            op: crate::hir::BinaryOp::Add,
            left,
            right,
        } = expression
        {
            return self.collect_string_concat_fragments(left, fragments)
                && self.collect_string_concat_fragments(right, fragments);
        }

        match expression {
            Expression::String(text) => {
                if let Some(EvalStringFragment::Static(existing)) = fragments.last_mut() {
                    existing.push_str(text);
                } else {
                    fragments.push(EvalStringFragment::Static(text.clone()));
                }
            }
            _ => fragments.push(EvalStringFragment::Dynamic),
        }

        true
    }

    fn is_reflect_construct_function(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> bool {
        let Some(first_argument) = arguments.first() else {
            return false;
        };
        let CallArgument::Expression(first_argument) = first_argument else {
            return false;
        };

        let targets_function = self.is_global_identifier(first_argument, "Function")
            || matches!(
                first_argument,
                Expression::Member { object, property }
                    if self.is_global_identifier(object, "globalThis")
                        && self.is_string_literal(property, "Function")
            );

        targets_function
            && matches!(
                callee,
                Expression::Member { object, property }
                    if self.is_global_identifier(object, "Reflect")
                        && self.is_string_literal(property, "construct")
            )
    }
}

enum EvalStringFragment {
    Static(String),
    Dynamic,
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

fn function_constructor_literal_source_parts(
    arguments: &[CallArgument],
) -> Option<(String, String)> {
    let parts = arguments
        .iter()
        .map(|argument| match argument {
            CallArgument::Expression(Expression::String(text)) => Some(text.clone()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;

    let Some((body_source, parameter_sources)) = parts.split_last() else {
        return Some((String::new(), String::new()));
    };

    Some((parameter_sources.join(","), body_source.clone()))
}

#[cfg(test)]
mod tests {
    use super::validate_refined_aot;
    use crate::frontend;

    #[test]
    fn rejects_builtin_eval() {
        let program = frontend::parse("eval('1');").unwrap();
        validate_refined_aot(&program).unwrap();
    }

    #[test]
    fn rejects_non_literal_direct_eval() {
        let program = frontend::parse(
            r#"
            let source = "1";
            eval(source);
            "#,
        )
        .unwrap();

        let error = validate_refined_aot(&program).unwrap_err();
        assert!(error.to_string().contains("compile-time string literal"));
    }

    #[test]
    fn allows_direct_eval_comment_patterns() {
        let program = frontend::parse(
            r#"
            var xx = String.fromCharCode(0x000A);
            eval("//var " + xx + "yy = -1");
            eval("/*var " + String.fromCharCode(0x0000) + "xx = 1*/");
            "#,
        )
        .unwrap();

        validate_refined_aot(&program).unwrap();
    }

    #[test]
    fn allows_static_function_constructor_literal_sources() {
        let program = frontend::parse("new Function('value', 'return value + 1;');").unwrap();
        validate_refined_aot(&program).unwrap();
    }

    #[test]
    fn rejects_dynamic_function_constructor() {
        let program = frontend::parse(
            r#"
            let body = "return 1;";
            new Function(body);
            "#,
        )
        .unwrap();
        let error = validate_refined_aot(&program).unwrap_err();
        assert!(error.to_string().contains("runtime source evaluation"));
    }

    #[test]
    fn rejects_realm_eval() {
        let program = frontend::parse("Realm.eval('1');").unwrap();
        let error = validate_refined_aot(&program).unwrap_err();
        assert!(error.to_string().contains("runtime source evaluation"));
    }

    #[test]
    fn allows_shadowed_eval_binding() {
        let program = frontend::parse(
            r#"
            function eval(value) {
              return value;
            }

            console.log(eval(1));
            "#,
        )
        .unwrap();

        validate_refined_aot(&program).unwrap();
    }

    #[test]
    fn allows_outer_scope_eval_shadowing_for_nested_functions() {
        let program = frontend::parse(
            r#"
            function outer() {
              let eval = 1;

              function inner() {
                return eval;
              }

              return inner();
            }

            console.log(outer());
            "#,
        )
        .unwrap();

        validate_refined_aot(&program).unwrap();
    }
}
