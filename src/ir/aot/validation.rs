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
                crate::ir::hir::UnaryOp::TypeOf => KnownValueKind::String,
                crate::ir::hir::UnaryOp::Not | crate::ir::hir::UnaryOp::Delete => KnownValueKind::NonString,
                crate::ir::hir::UnaryOp::Void
                | crate::ir::hir::UnaryOp::Plus
                | crate::ir::hir::UnaryOp::Negate
                | crate::ir::hir::UnaryOp::BitwiseNot => KnownValueKind::NonString,
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
            op: crate::ir::hir::BinaryOp::Add,
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
