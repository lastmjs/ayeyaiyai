use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn collect_parameter_get_iterator_names_from_expression(
        expression: &Expression,
        param_names: &HashSet<String>,
        consumed_names: &mut HashSet<String>,
    ) {
        match expression {
            Expression::GetIterator(value) => {
                if let Expression::Identifier(name) = value.as_ref()
                    && param_names.contains(name)
                {
                    consumed_names.insert(name.clone());
                }
                Self::collect_parameter_get_iterator_names_from_expression(
                    value,
                    param_names,
                    consumed_names,
                );
            }
            Expression::Member { object, property } => {
                Self::collect_parameter_get_iterator_names_from_expression(
                    object,
                    param_names,
                    consumed_names,
                );
                Self::collect_parameter_get_iterator_names_from_expression(
                    property,
                    param_names,
                    consumed_names,
                );
            }
            Expression::SuperMember { property } => {
                Self::collect_parameter_get_iterator_names_from_expression(
                    property,
                    param_names,
                    consumed_names,
                );
            }
            Expression::Assign { value, .. }
            | Expression::Await(value)
            | Expression::EnumerateKeys(value)
            | Expression::IteratorClose(value)
            | Expression::Unary {
                expression: value, ..
            } => Self::collect_parameter_get_iterator_names_from_expression(
                value,
                param_names,
                consumed_names,
            ),
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                Self::collect_parameter_get_iterator_names_from_expression(
                    object,
                    param_names,
                    consumed_names,
                );
                Self::collect_parameter_get_iterator_names_from_expression(
                    property,
                    param_names,
                    consumed_names,
                );
                Self::collect_parameter_get_iterator_names_from_expression(
                    value,
                    param_names,
                    consumed_names,
                );
            }
            Expression::AssignSuperMember { property, value } => {
                Self::collect_parameter_get_iterator_names_from_expression(
                    property,
                    param_names,
                    consumed_names,
                );
                Self::collect_parameter_get_iterator_names_from_expression(
                    value,
                    param_names,
                    consumed_names,
                );
            }
            Expression::Binary { left, right, .. } => {
                Self::collect_parameter_get_iterator_names_from_expression(
                    left,
                    param_names,
                    consumed_names,
                );
                Self::collect_parameter_get_iterator_names_from_expression(
                    right,
                    param_names,
                    consumed_names,
                );
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                Self::collect_parameter_get_iterator_names_from_expression(
                    condition,
                    param_names,
                    consumed_names,
                );
                Self::collect_parameter_get_iterator_names_from_expression(
                    then_expression,
                    param_names,
                    consumed_names,
                );
                Self::collect_parameter_get_iterator_names_from_expression(
                    else_expression,
                    param_names,
                    consumed_names,
                );
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    Self::collect_parameter_get_iterator_names_from_expression(
                        expression,
                        param_names,
                        consumed_names,
                    );
                }
            }
            Expression::Call { callee, arguments }
            | Expression::SuperCall { callee, arguments }
            | Expression::New { callee, arguments } => {
                Self::collect_parameter_get_iterator_names_from_expression(
                    callee,
                    param_names,
                    consumed_names,
                );
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            Self::collect_parameter_get_iterator_names_from_expression(
                                expression,
                                param_names,
                                consumed_names,
                            );
                        }
                    }
                }
            }
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                            Self::collect_parameter_get_iterator_names_from_expression(
                                expression,
                                param_names,
                                consumed_names,
                            );
                        }
                    }
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        ObjectEntry::Data { key, value } => {
                            Self::collect_parameter_get_iterator_names_from_expression(
                                key,
                                param_names,
                                consumed_names,
                            );
                            Self::collect_parameter_get_iterator_names_from_expression(
                                value,
                                param_names,
                                consumed_names,
                            );
                        }
                        ObjectEntry::Getter { key, getter } => {
                            Self::collect_parameter_get_iterator_names_from_expression(
                                key,
                                param_names,
                                consumed_names,
                            );
                            Self::collect_parameter_get_iterator_names_from_expression(
                                getter,
                                param_names,
                                consumed_names,
                            );
                        }
                        ObjectEntry::Setter { key, setter } => {
                            Self::collect_parameter_get_iterator_names_from_expression(
                                key,
                                param_names,
                                consumed_names,
                            );
                            Self::collect_parameter_get_iterator_names_from_expression(
                                setter,
                                param_names,
                                consumed_names,
                            );
                        }
                        ObjectEntry::Spread(expression) => {
                            Self::collect_parameter_get_iterator_names_from_expression(
                                expression,
                                param_names,
                                consumed_names,
                            );
                        }
                    }
                }
            }
            Expression::Identifier(_)
            | Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::This
            | Expression::NewTarget
            | Expression::Sent
            | Expression::Update { .. } => {}
        }
    }

    pub(in crate::backend::direct_wasm) fn collect_parameter_get_iterator_names_from_statements(
        statements: &[Statement],
        param_names: &HashSet<String>,
        consumed_names: &mut HashSet<String>,
    ) {
        for statement in statements {
            match statement {
                Statement::Block { body }
                | Statement::Labeled { body, .. }
                | Statement::With { body, .. } => {
                    Self::collect_parameter_get_iterator_names_from_statements(
                        body,
                        param_names,
                        consumed_names,
                    );
                }
                Statement::Expression(expression)
                | Statement::Return(expression)
                | Statement::Throw(expression)
                | Statement::Yield { value: expression }
                | Statement::YieldDelegate { value: expression } => {
                    Self::collect_parameter_get_iterator_names_from_expression(
                        expression,
                        param_names,
                        consumed_names,
                    );
                }
                Statement::Var { value, .. }
                | Statement::Let { value, .. }
                | Statement::Assign { value, .. } => {
                    Self::collect_parameter_get_iterator_names_from_expression(
                        value,
                        param_names,
                        consumed_names,
                    );
                }
                Statement::AssignMember {
                    object,
                    property,
                    value,
                } => {
                    Self::collect_parameter_get_iterator_names_from_expression(
                        object,
                        param_names,
                        consumed_names,
                    );
                    Self::collect_parameter_get_iterator_names_from_expression(
                        property,
                        param_names,
                        consumed_names,
                    );
                    Self::collect_parameter_get_iterator_names_from_expression(
                        value,
                        param_names,
                        consumed_names,
                    );
                }
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    Self::collect_parameter_get_iterator_names_from_expression(
                        condition,
                        param_names,
                        consumed_names,
                    );
                    Self::collect_parameter_get_iterator_names_from_statements(
                        then_branch,
                        param_names,
                        consumed_names,
                    );
                    Self::collect_parameter_get_iterator_names_from_statements(
                        else_branch,
                        param_names,
                        consumed_names,
                    );
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
                    Self::collect_parameter_get_iterator_names_from_expression(
                        condition,
                        param_names,
                        consumed_names,
                    );
                    if let Some(break_hook) = break_hook {
                        Self::collect_parameter_get_iterator_names_from_expression(
                            break_hook,
                            param_names,
                            consumed_names,
                        );
                    }
                    Self::collect_parameter_get_iterator_names_from_statements(
                        body,
                        param_names,
                        consumed_names,
                    );
                }
                Statement::For {
                    init,
                    condition,
                    update,
                    break_hook,
                    body,
                    ..
                } => {
                    Self::collect_parameter_get_iterator_names_from_statements(
                        init,
                        param_names,
                        consumed_names,
                    );
                    if let Some(condition) = condition {
                        Self::collect_parameter_get_iterator_names_from_expression(
                            condition,
                            param_names,
                            consumed_names,
                        );
                    }
                    if let Some(update) = update {
                        Self::collect_parameter_get_iterator_names_from_expression(
                            update,
                            param_names,
                            consumed_names,
                        );
                    }
                    if let Some(break_hook) = break_hook {
                        Self::collect_parameter_get_iterator_names_from_expression(
                            break_hook,
                            param_names,
                            consumed_names,
                        );
                    }
                    Self::collect_parameter_get_iterator_names_from_statements(
                        body,
                        param_names,
                        consumed_names,
                    );
                }
                Statement::Try {
                    body,
                    catch_setup,
                    catch_body,
                    ..
                } => {
                    Self::collect_parameter_get_iterator_names_from_statements(
                        body,
                        param_names,
                        consumed_names,
                    );
                    Self::collect_parameter_get_iterator_names_from_statements(
                        catch_setup,
                        param_names,
                        consumed_names,
                    );
                    Self::collect_parameter_get_iterator_names_from_statements(
                        catch_body,
                        param_names,
                        consumed_names,
                    );
                }
                Statement::Switch {
                    discriminant,
                    cases,
                    ..
                } => {
                    Self::collect_parameter_get_iterator_names_from_expression(
                        discriminant,
                        param_names,
                        consumed_names,
                    );
                    for case in cases {
                        if let Some(test) = &case.test {
                            Self::collect_parameter_get_iterator_names_from_expression(
                                test,
                                param_names,
                                consumed_names,
                            );
                        }
                        Self::collect_parameter_get_iterator_names_from_statements(
                            &case.body,
                            param_names,
                            consumed_names,
                        );
                    }
                }
                Statement::Print { values } => {
                    for value in values {
                        Self::collect_parameter_get_iterator_names_from_expression(
                            value,
                            param_names,
                            consumed_names,
                        );
                    }
                }
                Statement::Break { .. } | Statement::Continue { .. } => {}
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn user_function_parameter_iterator_consumption_indices(
        &self,
        user_function: &UserFunction,
    ) -> HashSet<usize> {
        let Some(function) = self.resolve_registered_function_declaration(&user_function.name)
        else {
            return HashSet::new();
        };
        let param_names = user_function.params.iter().cloned().collect::<HashSet<_>>();
        let mut consumed_names = HashSet::new();
        Self::collect_parameter_get_iterator_names_from_statements(
            &function.body,
            &param_names,
            &mut consumed_names,
        );
        user_function
            .params
            .iter()
            .enumerate()
            .filter_map(|(index, param_name)| consumed_names.contains(param_name).then_some(index))
            .collect()
    }

    pub(in crate::backend::direct_wasm) fn close_local_iterator_binding(&mut self, name: &str) {
        let Some(mut binding) = self.local_array_iterator_bindings.get(name).cloned() else {
            return;
        };
        let (closed_state, closed_static_index) = match &binding.source {
            IteratorSourceKind::StaticArray {
                values,
                length_local,
                runtime_name,
                ..
            } => {
                let closed_static_index = if length_local.is_none() && runtime_name.is_none() {
                    Some(values.len().saturating_add(1))
                } else {
                    None
                };
                (i32::MAX, closed_static_index)
            }
            IteratorSourceKind::SimpleGenerator { steps, .. } => {
                let closed_index = steps.len().saturating_add(1);
                (closed_index as i32, Some(closed_index))
            }
            IteratorSourceKind::TypedArrayView { .. }
            | IteratorSourceKind::DirectArguments { .. } => (i32::MAX, None),
        };
        self.push_i32_const(closed_state);
        self.push_local_set(binding.index_local);
        binding.static_index = closed_static_index;
        self.local_array_iterator_bindings
            .insert(name.to_string(), binding);
    }

    pub(in crate::backend::direct_wasm) fn sync_argument_iterator_bindings_for_user_call(
        &mut self,
        user_function: &UserFunction,
        arguments: &[Expression],
    ) {
        let consumed_indices =
            self.user_function_parameter_iterator_consumption_indices(user_function);
        if consumed_indices.is_empty() {
            return;
        }
        for (index, argument) in arguments.iter().enumerate() {
            if !consumed_indices.contains(&index) {
                continue;
            }
            let Some(name) = (match argument {
                Expression::Identifier(name) => Some(name.clone()),
                _ => match self.resolve_bound_alias_expression(argument) {
                    Some(Expression::Identifier(name)) => Some(name),
                    _ => None,
                },
            }) else {
                continue;
            };
            let Some(binding_name) = self.resolve_local_array_iterator_binding_name(&name) else {
                continue;
            };
            self.close_local_iterator_binding(&binding_name);
        }
    }

    pub(in crate::backend::direct_wasm) fn collect_iterator_close_binding_names_from_statements(
        statements: &[Statement],
        names: &mut Vec<String>,
    ) {
        fn collect_from_expression(expression: &Expression, names: &mut Vec<String>) {
            match expression {
                Expression::IteratorClose(value) => {
                    if let Expression::Identifier(name) = value.as_ref() {
                        names.push(name.clone());
                    }
                    collect_from_expression(value, names);
                }
                Expression::Member { object, property } => {
                    collect_from_expression(object, names);
                    collect_from_expression(property, names);
                }
                Expression::SuperMember { property } => {
                    collect_from_expression(property, names);
                }
                Expression::Assign { value, .. }
                | Expression::Await(value)
                | Expression::EnumerateKeys(value)
                | Expression::GetIterator(value)
                | Expression::Unary {
                    expression: value, ..
                } => collect_from_expression(value, names),
                Expression::AssignMember {
                    object,
                    property,
                    value,
                } => {
                    collect_from_expression(object, names);
                    collect_from_expression(property, names);
                    collect_from_expression(value, names);
                }
                Expression::AssignSuperMember { property, value } => {
                    collect_from_expression(property, names);
                    collect_from_expression(value, names);
                }
                Expression::Binary { left, right, .. } => {
                    collect_from_expression(left, names);
                    collect_from_expression(right, names);
                }
                Expression::Conditional {
                    condition,
                    then_expression,
                    else_expression,
                } => {
                    collect_from_expression(condition, names);
                    collect_from_expression(then_expression, names);
                    collect_from_expression(else_expression, names);
                }
                Expression::Sequence(expressions) => {
                    for expression in expressions {
                        collect_from_expression(expression, names);
                    }
                }
                Expression::Call { callee, arguments }
                | Expression::SuperCall { callee, arguments }
                | Expression::New { callee, arguments } => {
                    collect_from_expression(callee, names);
                    for argument in arguments {
                        match argument {
                            CallArgument::Expression(expression)
                            | CallArgument::Spread(expression) => {
                                collect_from_expression(expression, names);
                            }
                        }
                    }
                }
                Expression::Array(elements) => {
                    for element in elements {
                        match element {
                            ArrayElement::Expression(expression)
                            | ArrayElement::Spread(expression) => {
                                collect_from_expression(expression, names);
                            }
                        }
                    }
                }
                Expression::Object(entries) => {
                    for entry in entries {
                        match entry {
                            ObjectEntry::Data { key, value } => {
                                collect_from_expression(key, names);
                                collect_from_expression(value, names);
                            }
                            ObjectEntry::Getter { key, getter } => {
                                collect_from_expression(key, names);
                                collect_from_expression(getter, names);
                            }
                            ObjectEntry::Setter { key, setter } => {
                                collect_from_expression(key, names);
                                collect_from_expression(setter, names);
                            }
                            ObjectEntry::Spread(expression) => {
                                collect_from_expression(expression, names);
                            }
                        }
                    }
                }
                Expression::Identifier(_)
                | Expression::Number(_)
                | Expression::BigInt(_)
                | Expression::String(_)
                | Expression::Bool(_)
                | Expression::Null
                | Expression::Undefined
                | Expression::This
                | Expression::NewTarget
                | Expression::Sent
                | Expression::Update { .. } => {}
            }
        }

        for statement in statements {
            match statement {
                Statement::Block { body }
                | Statement::Labeled { body, .. }
                | Statement::With { body, .. } => {
                    Self::collect_iterator_close_binding_names_from_statements(body, names);
                }
                Statement::Expression(expression)
                | Statement::Return(expression)
                | Statement::Throw(expression)
                | Statement::Yield { value: expression }
                | Statement::YieldDelegate { value: expression } => {
                    collect_from_expression(expression, names);
                }
                Statement::Var { value, .. }
                | Statement::Let { value, .. }
                | Statement::Assign { value, .. } => {
                    collect_from_expression(value, names);
                }
                Statement::AssignMember {
                    object,
                    property,
                    value,
                } => {
                    collect_from_expression(object, names);
                    collect_from_expression(property, names);
                    collect_from_expression(value, names);
                }
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    collect_from_expression(condition, names);
                    Self::collect_iterator_close_binding_names_from_statements(then_branch, names);
                    Self::collect_iterator_close_binding_names_from_statements(else_branch, names);
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
                    collect_from_expression(condition, names);
                    if let Some(break_hook) = break_hook {
                        collect_from_expression(break_hook, names);
                    }
                    Self::collect_iterator_close_binding_names_from_statements(body, names);
                }
                Statement::For {
                    init,
                    condition,
                    update,
                    break_hook,
                    body,
                    ..
                } => {
                    Self::collect_iterator_close_binding_names_from_statements(init, names);
                    if let Some(condition) = condition {
                        collect_from_expression(condition, names);
                    }
                    if let Some(update) = update {
                        collect_from_expression(update, names);
                    }
                    if let Some(break_hook) = break_hook {
                        collect_from_expression(break_hook, names);
                    }
                    Self::collect_iterator_close_binding_names_from_statements(body, names);
                }
                Statement::Try {
                    body,
                    catch_setup,
                    catch_body,
                    ..
                } => {
                    Self::collect_iterator_close_binding_names_from_statements(body, names);
                    Self::collect_iterator_close_binding_names_from_statements(catch_setup, names);
                    Self::collect_iterator_close_binding_names_from_statements(catch_body, names);
                }
                Statement::Switch {
                    discriminant,
                    cases,
                    ..
                } => {
                    collect_from_expression(discriminant, names);
                    for case in cases {
                        if let Some(test) = &case.test {
                            collect_from_expression(test, names);
                        }
                        Self::collect_iterator_close_binding_names_from_statements(
                            &case.body, names,
                        );
                    }
                }
                Statement::Print { values } => {
                    for value in values {
                        collect_from_expression(value, names);
                    }
                }
                Statement::Break { .. } | Statement::Continue { .. } => {}
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn find_iterator_source_expression_in_statements(
        statements: &[Statement],
        iterator_name: &str,
    ) -> Option<Expression> {
        for statement in statements {
            match statement {
                Statement::Block { body }
                | Statement::Labeled { body, .. }
                | Statement::With { body, .. } => {
                    if let Some(iterated) =
                        Self::find_iterator_source_expression_in_statements(body, iterator_name)
                    {
                        return Some(iterated);
                    }
                }
                Statement::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    if let Some(iterated) = Self::find_iterator_source_expression_in_statements(
                        then_branch,
                        iterator_name,
                    ) {
                        return Some(iterated);
                    }
                    if let Some(iterated) = Self::find_iterator_source_expression_in_statements(
                        else_branch,
                        iterator_name,
                    ) {
                        return Some(iterated);
                    }
                }
                Statement::Try {
                    body,
                    catch_setup,
                    catch_body,
                    ..
                } => {
                    if let Some(iterated) =
                        Self::find_iterator_source_expression_in_statements(body, iterator_name)
                    {
                        return Some(iterated);
                    }
                    if let Some(iterated) = Self::find_iterator_source_expression_in_statements(
                        catch_setup,
                        iterator_name,
                    ) {
                        return Some(iterated);
                    }
                    if let Some(iterated) = Self::find_iterator_source_expression_in_statements(
                        catch_body,
                        iterator_name,
                    ) {
                        return Some(iterated);
                    }
                }
                Statement::Switch { cases, .. } => {
                    for case in cases {
                        if let Some(iterated) = Self::find_iterator_source_expression_in_statements(
                            &case.body,
                            iterator_name,
                        ) {
                            return Some(iterated);
                        }
                    }
                }
                Statement::For { init, body, .. } => {
                    if let Some(iterated) =
                        Self::find_iterator_source_expression_in_statements(init, iterator_name)
                    {
                        return Some(iterated);
                    }
                    if let Some(iterated) =
                        Self::find_iterator_source_expression_in_statements(body, iterator_name)
                    {
                        return Some(iterated);
                    }
                }
                Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
                    if let Some(iterated) =
                        Self::find_iterator_source_expression_in_statements(body, iterator_name)
                    {
                        return Some(iterated);
                    }
                }
                Statement::Var { name, value }
                | Statement::Let { name, value, .. }
                | Statement::Assign { name, value }
                    if name == iterator_name =>
                {
                    if let Expression::GetIterator(iterated) = value {
                        return Some((**iterated).clone());
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_iterator_close_return_binding_in_function(
        &self,
        iterator_name: &str,
        current_function_name: Option<&str>,
    ) -> Option<LocalFunctionBinding> {
        let function_name = current_function_name?;
        let function = self.resolve_registered_function_declaration(function_name)?;
        let iterated =
            Self::find_iterator_source_expression_in_statements(&function.body, iterator_name)?;
        let iterator_call = Expression::Call {
            callee: Box::new(Expression::Member {
                object: Box::new(iterated),
                property: Box::new(symbol_iterator_expression()),
            }),
            arguments: Vec::new(),
        };
        self.inherited_member_function_bindings(&iterator_call)
            .into_iter()
            .find(|binding| binding.property == "return")
            .map(|binding| binding.binding)
    }

    pub(in crate::backend::direct_wasm) fn collect_user_function_updated_nonlocal_bindings(
        &self,
        user_function: &UserFunction,
    ) -> HashSet<String> {
        fn collect_updated_names_from_statement(
            statement: &Statement,
            names: &mut HashSet<String>,
        ) {
            match statement {
                Statement::Block { body }
                | Statement::Labeled { body, .. }
                | Statement::With { body, .. } => {
                    for statement in body {
                        collect_updated_names_from_statement(statement, names);
                    }
                }
                Statement::Expression(expression)
                | Statement::Return(expression)
                | Statement::Throw(expression)
                | Statement::Yield { value: expression }
                | Statement::YieldDelegate { value: expression } => {
                    collect_updated_names_from_expression(expression, names);
                }
                Statement::Var { value, .. } | Statement::Let { value, .. } => {
                    collect_updated_names_from_expression(value, names);
                }
                Statement::Print { values } => {
                    for value in values {
                        collect_updated_names_from_expression(value, names);
                    }
                }
                Statement::Assign { value, .. } => {
                    collect_updated_names_from_expression(value, names);
                }
                Statement::AssignMember {
                    object,
                    property,
                    value,
                } => {
                    collect_updated_names_from_expression(object, names);
                    collect_updated_names_from_expression(property, names);
                    collect_updated_names_from_expression(value, names);
                }
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    collect_updated_names_from_expression(condition, names);
                    for statement in then_branch {
                        collect_updated_names_from_statement(statement, names);
                    }
                    for statement in else_branch {
                        collect_updated_names_from_statement(statement, names);
                    }
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
                    collect_updated_names_from_expression(condition, names);
                    if let Some(break_hook) = break_hook {
                        collect_updated_names_from_expression(break_hook, names);
                    }
                    for statement in body {
                        collect_updated_names_from_statement(statement, names);
                    }
                }
                Statement::For {
                    init,
                    condition,
                    update,
                    break_hook,
                    body,
                    ..
                } => {
                    for statement in init {
                        collect_updated_names_from_statement(statement, names);
                    }
                    if let Some(condition) = condition {
                        collect_updated_names_from_expression(condition, names);
                    }
                    if let Some(update) = update {
                        collect_updated_names_from_expression(update, names);
                    }
                    if let Some(break_hook) = break_hook {
                        collect_updated_names_from_expression(break_hook, names);
                    }
                    for statement in body {
                        collect_updated_names_from_statement(statement, names);
                    }
                }
                Statement::Try {
                    body,
                    catch_setup,
                    catch_body,
                    ..
                } => {
                    for statement in body {
                        collect_updated_names_from_statement(statement, names);
                    }
                    for statement in catch_setup {
                        collect_updated_names_from_statement(statement, names);
                    }
                    for statement in catch_body {
                        collect_updated_names_from_statement(statement, names);
                    }
                }
                Statement::Switch {
                    discriminant,
                    cases,
                    ..
                } => {
                    collect_updated_names_from_expression(discriminant, names);
                    for case in cases {
                        if let Some(test) = &case.test {
                            collect_updated_names_from_expression(test, names);
                        }
                        for statement in &case.body {
                            collect_updated_names_from_statement(statement, names);
                        }
                    }
                }
                Statement::Break { .. } | Statement::Continue { .. } => {}
            }
        }

        fn collect_updated_names_from_expression(
            expression: &Expression,
            names: &mut HashSet<String>,
        ) {
            match expression {
                Expression::Update { name, .. } => {
                    names.insert(name.clone());
                }
                Expression::Member { object, property } => {
                    collect_updated_names_from_expression(object, names);
                    collect_updated_names_from_expression(property, names);
                }
                Expression::SuperMember { property } => {
                    collect_updated_names_from_expression(property, names);
                }
                Expression::Assign { value, .. }
                | Expression::Await(value)
                | Expression::EnumerateKeys(value)
                | Expression::GetIterator(value)
                | Expression::IteratorClose(value)
                | Expression::Unary {
                    expression: value, ..
                } => collect_updated_names_from_expression(value, names),
                Expression::AssignMember {
                    object,
                    property,
                    value,
                } => {
                    collect_updated_names_from_expression(object, names);
                    collect_updated_names_from_expression(property, names);
                    collect_updated_names_from_expression(value, names);
                }
                Expression::AssignSuperMember { property, value } => {
                    collect_updated_names_from_expression(property, names);
                    collect_updated_names_from_expression(value, names);
                }
                Expression::Binary { left, right, .. } => {
                    collect_updated_names_from_expression(left, names);
                    collect_updated_names_from_expression(right, names);
                }
                Expression::Conditional {
                    condition,
                    then_expression,
                    else_expression,
                } => {
                    collect_updated_names_from_expression(condition, names);
                    collect_updated_names_from_expression(then_expression, names);
                    collect_updated_names_from_expression(else_expression, names);
                }
                Expression::Sequence(expressions) => {
                    for expression in expressions {
                        collect_updated_names_from_expression(expression, names);
                    }
                }
                Expression::Call { callee, arguments }
                | Expression::SuperCall { callee, arguments }
                | Expression::New { callee, arguments } => {
                    collect_updated_names_from_expression(callee, names);
                    for argument in arguments {
                        match argument {
                            CallArgument::Expression(expression)
                            | CallArgument::Spread(expression) => {
                                collect_updated_names_from_expression(expression, names);
                            }
                        }
                    }
                }
                Expression::Array(elements) => {
                    for element in elements {
                        match element {
                            ArrayElement::Expression(expression)
                            | ArrayElement::Spread(expression) => {
                                collect_updated_names_from_expression(expression, names);
                            }
                        }
                    }
                }
                Expression::Object(entries) => {
                    for entry in entries {
                        match entry {
                            ObjectEntry::Data { key, value } => {
                                collect_updated_names_from_expression(key, names);
                                collect_updated_names_from_expression(value, names);
                            }
                            ObjectEntry::Getter { key, getter } => {
                                collect_updated_names_from_expression(key, names);
                                collect_updated_names_from_expression(getter, names);
                            }
                            ObjectEntry::Setter { key, setter } => {
                                collect_updated_names_from_expression(key, names);
                                collect_updated_names_from_expression(setter, names);
                            }
                            ObjectEntry::Spread(expression) => {
                                collect_updated_names_from_expression(expression, names);
                            }
                        }
                    }
                }
                Expression::Identifier(_)
                | Expression::Number(_)
                | Expression::BigInt(_)
                | Expression::String(_)
                | Expression::Bool(_)
                | Expression::Null
                | Expression::Undefined
                | Expression::This
                | Expression::NewTarget
                | Expression::Sent => {}
            }
        }

        let Some(function) = self.resolve_registered_function_declaration(&user_function.name)
        else {
            return HashSet::new();
        };
        let mut names = HashSet::new();
        for statement in &function.body {
            collect_updated_names_from_statement(statement, &mut names);
        }
        names.retain(|name| {
            let source_name = scoped_binding_source_name(name).unwrap_or(name);
            !user_function.scope_bindings.contains(source_name)
        });
        names
    }

    pub(in crate::backend::direct_wasm) fn collect_static_direct_eval_assigned_nonlocal_names_from_statement(
        &self,
        statement: &Statement,
        current_function_name: Option<&str>,
        names: &mut HashSet<String>,
    ) {
        match statement {
            Statement::Block { body } | Statement::Labeled { body, .. } => {
                for statement in body {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                        statement,
                        current_function_name,
                        names,
                    );
                }
            }
            Statement::Var { value, .. }
            | Statement::Let { value, .. }
            | Statement::Assign { value, .. }
            | Statement::Expression(value)
            | Statement::Throw(value)
            | Statement::Return(value)
            | Statement::Yield { value }
            | Statement::YieldDelegate { value } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    value,
                    current_function_name,
                    names,
                );
            }
            Statement::Print { values } => {
                for value in values {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                        value,
                        current_function_name,
                        names,
                    );
                }
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    object,
                    current_function_name,
                    names,
                );
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    property,
                    current_function_name,
                    names,
                );
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    value,
                    current_function_name,
                    names,
                );
            }
            Statement::With { object, body } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    object,
                    current_function_name,
                    names,
                );
                for statement in body {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                        statement,
                        current_function_name,
                        names,
                    );
                }
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    condition,
                    current_function_name,
                    names,
                );
                for statement in then_branch {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                        statement,
                        current_function_name,
                        names,
                    );
                }
                for statement in else_branch {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                        statement,
                        current_function_name,
                        names,
                    );
                }
            }
            Statement::Try {
                body,
                catch_setup,
                catch_body,
                ..
            } => {
                for statement in body {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                        statement,
                        current_function_name,
                        names,
                    );
                }
                for statement in catch_setup {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                        statement,
                        current_function_name,
                        names,
                    );
                }
                for statement in catch_body {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                        statement,
                        current_function_name,
                        names,
                    );
                }
            }
            Statement::Switch {
                discriminant,
                cases,
                ..
            } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    discriminant,
                    current_function_name,
                    names,
                );
                for case in cases {
                    if let Some(test) = &case.test {
                        self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                            test,
                            current_function_name,
                            names,
                        );
                    }
                    for statement in &case.body {
                        self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                            statement,
                            current_function_name,
                            names,
                        );
                    }
                }
            }
            Statement::For {
                init,
                condition,
                update,
                break_hook,
                body,
                ..
            } => {
                for statement in init {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                        statement,
                        current_function_name,
                        names,
                    );
                }
                if let Some(condition) = condition {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                        condition,
                        current_function_name,
                        names,
                    );
                }
                if let Some(update) = update {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                        update,
                        current_function_name,
                        names,
                    );
                }
                if let Some(break_hook) = break_hook {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                        break_hook,
                        current_function_name,
                        names,
                    );
                }
                for statement in body {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                        statement,
                        current_function_name,
                        names,
                    );
                }
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
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    condition,
                    current_function_name,
                    names,
                );
                if let Some(break_hook) = break_hook {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                        break_hook,
                        current_function_name,
                        names,
                    );
                }
                for statement in body {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                        statement,
                        current_function_name,
                        names,
                    );
                }
            }
            Statement::Break { .. } | Statement::Continue { .. } => {}
        }
    }

    pub(in crate::backend::direct_wasm) fn collect_static_direct_eval_assigned_nonlocal_names_from_expression(
        &self,
        expression: &Expression,
        current_function_name: Option<&str>,
        names: &mut HashSet<String>,
    ) {
        match expression {
            Expression::Call { callee, arguments } if matches!(callee.as_ref(), Expression::Identifier(name) if name == "eval") =>
            {
                if let Some(CallArgument::Expression(Expression::String(source))) =
                    arguments.first()
                    && let Some(mut eval_program) = self
                        .module
                        .parse_static_eval_program_in_context(source, current_function_name)
                {
                    namespace_eval_program_internal_function_names(
                        &mut eval_program,
                        current_function_name,
                        source,
                    );
                    self.normalize_eval_scoped_bindings_to_source_names(&mut eval_program);
                    let eval_local_function_declarations = if eval_program.strict {
                        HashMap::new()
                    } else {
                        collect_eval_local_function_declarations(
                            &eval_program.statements,
                            &eval_program
                                .functions
                                .iter()
                                .filter(|function| is_eval_local_function_candidate(function))
                                .map(|function| function.name.clone())
                                .collect::<HashSet<_>>(),
                        )
                    };
                    let mut eval_assigned_names = HashSet::new();
                    for statement in eval_program.statements.iter().filter(|statement| {
                        !is_eval_local_function_declaration_statement(
                            statement,
                            &eval_local_function_declarations,
                        )
                    }) {
                        collect_assigned_binding_names_from_statement(
                            statement,
                            &mut eval_assigned_names,
                        );
                        self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                            statement,
                            current_function_name,
                            names,
                        );
                    }
                    let mut declared_bindings = collect_declared_bindings_from_statements_recursive(
                        &eval_program.statements,
                    );
                    for function in &eval_program.functions {
                        declared_bindings.insert(function.name.clone());
                        if let Some(binding) = &function.top_level_binding {
                            declared_bindings.insert(binding.clone());
                        }
                        if let Some(binding) = &function.self_binding {
                            declared_bindings.insert(binding.clone());
                        }
                        for parameter in &function.params {
                            declared_bindings.insert(parameter.name.clone());
                        }
                    }
                    for name in eval_assigned_names {
                        let source_name = scoped_binding_source_name(&name).unwrap_or(&name);
                        if declared_bindings.contains(source_name) {
                            continue;
                        }
                        names.insert(source_name.to_string());
                    }
                }
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                                expression,
                                current_function_name,
                                names,
                            );
                        }
                    }
                }
            }
            Expression::Member { object, property } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    object,
                    current_function_name,
                    names,
                );
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    property,
                    current_function_name,
                    names,
                );
            }
            Expression::SuperMember { property }
            | Expression::Await(property)
            | Expression::EnumerateKeys(property)
            | Expression::GetIterator(property)
            | Expression::IteratorClose(property)
            | Expression::Unary {
                expression: property,
                ..
            }
            | Expression::Assign {
                value: property, ..
            } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    property,
                    current_function_name,
                    names,
                );
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    object,
                    current_function_name,
                    names,
                );
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    property,
                    current_function_name,
                    names,
                );
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    value,
                    current_function_name,
                    names,
                );
            }
            Expression::AssignSuperMember { property, value } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    property,
                    current_function_name,
                    names,
                );
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    value,
                    current_function_name,
                    names,
                );
            }
            Expression::Binary { left, right, .. } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    left,
                    current_function_name,
                    names,
                );
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    right,
                    current_function_name,
                    names,
                );
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    condition,
                    current_function_name,
                    names,
                );
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    then_expression,
                    current_function_name,
                    names,
                );
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    else_expression,
                    current_function_name,
                    names,
                );
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                        expression,
                        current_function_name,
                        names,
                    );
                }
            }
            Expression::SuperCall { callee, arguments } | Expression::New { callee, arguments } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    callee,
                    current_function_name,
                    names,
                );
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                                expression,
                                current_function_name,
                                names,
                            );
                        }
                    }
                }
            }
            Expression::Call { callee, arguments } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    callee,
                    current_function_name,
                    names,
                );
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                                expression,
                                current_function_name,
                                names,
                            );
                        }
                    }
                }
            }
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                            self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                                expression,
                                current_function_name,
                                names,
                            );
                        }
                    }
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        ObjectEntry::Data { key, value } => {
                            self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                                key,
                                current_function_name,
                                names,
                            );
                            self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                                value,
                                current_function_name,
                                names,
                            );
                        }
                        ObjectEntry::Getter { key, getter } => {
                            self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                                key,
                                current_function_name,
                                names,
                            );
                            self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                                getter,
                                current_function_name,
                                names,
                            );
                        }
                        ObjectEntry::Setter { key, setter } => {
                            self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                                key,
                                current_function_name,
                                names,
                            );
                            self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                                setter,
                                current_function_name,
                                names,
                            );
                        }
                        ObjectEntry::Spread(expression) => {
                            self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                                expression,
                                current_function_name,
                                names,
                            );
                        }
                    }
                }
            }
            Expression::Identifier(_)
            | Expression::Update { .. }
            | Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::This
            | Expression::NewTarget
            | Expression::Sent => {}
        }
    }

    pub(in crate::backend::direct_wasm) fn collect_user_function_assigned_nonlocal_bindings(
        &self,
        user_function: &UserFunction,
    ) -> HashSet<String> {
        let Some(function) = self.resolve_registered_function_declaration(&user_function.name)
        else {
            return HashSet::new();
        };
        let mut names = HashSet::new();
        for statement in &function.body {
            collect_assigned_binding_names_from_statement(statement, &mut names);
            self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                statement,
                Some(&user_function.name),
                &mut names,
            );
        }
        names.retain(|name| {
            let source_name = scoped_binding_source_name(name).unwrap_or(name);
            !user_function.scope_bindings.contains(source_name)
        });
        names
    }

    pub(in crate::backend::direct_wasm) fn collect_user_function_call_effect_nonlocal_bindings(
        &self,
        user_function: &UserFunction,
    ) -> HashSet<String> {
        let mut visited = HashSet::new();
        self.collect_user_function_call_effect_nonlocal_bindings_for_name(
            &user_function.name,
            &mut visited,
        )
    }

    pub(in crate::backend::direct_wasm) fn collect_user_function_call_effect_nonlocal_bindings_for_name(
        &self,
        function_name: &str,
        visited: &mut HashSet<String>,
    ) -> HashSet<String> {
        if !visited.insert(function_name.to_string()) {
            return HashSet::new();
        }
        let Some(user_function) = self.module.user_function_map.get(function_name) else {
            return HashSet::new();
        };
        let mut names = self.collect_user_function_assigned_nonlocal_bindings(user_function);
        let Some(function) = self.resolve_registered_function_declaration(function_name) else {
            return names;
        };
        for statement in &function.body {
            self.collect_statement_call_effect_nonlocal_bindings(
                statement,
                Some(function_name),
                &mut names,
                visited,
            );
        }
        names
    }

    pub(in crate::backend::direct_wasm) fn collect_statement_call_effect_nonlocal_bindings(
        &self,
        statement: &Statement,
        current_function_name: Option<&str>,
        names: &mut HashSet<String>,
        visited: &mut HashSet<String>,
    ) {
        match statement {
            Statement::Expression(expression) | Statement::Return(expression) => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    expression,
                    current_function_name,
                    names,
                    visited,
                );
            }
            Statement::Throw(expression) => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    expression,
                    current_function_name,
                    names,
                    visited,
                );
            }
            Statement::Var { value, .. }
            | Statement::Let { value, .. }
            | Statement::Assign { value, .. } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    value,
                    current_function_name,
                    names,
                    visited,
                );
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    &Expression::AssignMember {
                        object: Box::new(object.clone()),
                        property: Box::new(property.clone()),
                        value: Box::new(value.clone()),
                    },
                    current_function_name,
                    names,
                    visited,
                );
            }
            Statement::Block { body } => {
                for statement in body {
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
                    );
                }
            }
            Statement::Labeled { body, .. } => {
                for statement in body {
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
                    );
                }
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    condition,
                    current_function_name,
                    names,
                    visited,
                );
                for statement in then_branch {
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
                    );
                }
                for statement in else_branch {
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
                    );
                }
            }
            Statement::Try {
                body,
                catch_setup,
                catch_body,
                ..
            } => {
                for statement in body {
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
                    );
                }
                for statement in catch_setup {
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
                    );
                }
                for statement in catch_body {
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
                    );
                }
            }
            Statement::Switch {
                discriminant,
                cases,
                ..
            } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    discriminant,
                    current_function_name,
                    names,
                    visited,
                );
                for case in cases {
                    if let Some(test) = &case.test {
                        self.collect_expression_call_effect_nonlocal_bindings(
                            test,
                            current_function_name,
                            names,
                            visited,
                        );
                    }
                    for statement in &case.body {
                        self.collect_statement_call_effect_nonlocal_bindings(
                            statement,
                            current_function_name,
                            names,
                            visited,
                        );
                    }
                }
            }
            Statement::For {
                init,
                condition,
                update,
                break_hook,
                body,
                ..
            } => {
                for statement in init {
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
                    );
                }
                if let Some(condition) = condition {
                    self.collect_expression_call_effect_nonlocal_bindings(
                        condition,
                        current_function_name,
                        names,
                        visited,
                    );
                }
                if let Some(update) = update {
                    self.collect_expression_call_effect_nonlocal_bindings(
                        update,
                        current_function_name,
                        names,
                        visited,
                    );
                }
                if let Some(break_hook) = break_hook {
                    self.collect_expression_call_effect_nonlocal_bindings(
                        break_hook,
                        current_function_name,
                        names,
                        visited,
                    );
                }
                for statement in body {
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
                    );
                }
            }
            Statement::With { object, body } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    object,
                    current_function_name,
                    names,
                    visited,
                );
                for statement in body {
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
                    );
                }
            }
            Statement::While {
                condition,
                break_hook,
                body,
                ..
            } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    condition,
                    current_function_name,
                    names,
                    visited,
                );
                if let Some(break_hook) = break_hook {
                    self.collect_expression_call_effect_nonlocal_bindings(
                        break_hook,
                        current_function_name,
                        names,
                        visited,
                    );
                }
                for statement in body {
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
                    );
                }
            }
            Statement::DoWhile {
                condition,
                break_hook,
                body,
                ..
            } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    condition,
                    current_function_name,
                    names,
                    visited,
                );
                if let Some(break_hook) = break_hook {
                    self.collect_expression_call_effect_nonlocal_bindings(
                        break_hook,
                        current_function_name,
                        names,
                        visited,
                    );
                }
                for statement in body {
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
                    );
                }
            }
            Statement::Print { values } => {
                for value in values {
                    self.collect_expression_call_effect_nonlocal_bindings(
                        value,
                        current_function_name,
                        names,
                        visited,
                    );
                }
            }
            Statement::Yield { value } | Statement::YieldDelegate { value } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    value,
                    current_function_name,
                    names,
                    visited,
                );
            }
            Statement::Break { .. } | Statement::Continue { .. } => {}
        }
    }

    pub(in crate::backend::direct_wasm) fn collect_expression_call_effect_nonlocal_bindings(
        &self,
        expression: &Expression,
        current_function_name: Option<&str>,
        names: &mut HashSet<String>,
        visited: &mut HashSet<String>,
    ) {
        match expression {
            Expression::Call { callee, arguments }
            | Expression::SuperCall { callee, arguments }
            | Expression::New { callee, arguments } => {
                if let Some(LocalFunctionBinding::User(function_name)) = self
                    .resolve_function_binding_from_expression_with_context(
                        callee,
                        current_function_name,
                    )
                {
                    names.extend(
                        self.collect_user_function_call_effect_nonlocal_bindings_for_name(
                            &function_name,
                            visited,
                        ),
                    );
                }
                self.collect_expression_call_effect_nonlocal_bindings(
                    callee,
                    current_function_name,
                    names,
                    visited,
                );
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.collect_expression_call_effect_nonlocal_bindings(
                                expression,
                                current_function_name,
                                names,
                                visited,
                            );
                        }
                    }
                }
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                if let Some(LocalFunctionBinding::User(function_name)) =
                    self.resolve_member_setter_binding(object, property)
                {
                    names.extend(
                        self.collect_user_function_call_effect_nonlocal_bindings_for_name(
                            &function_name,
                            visited,
                        ),
                    );
                }
                self.collect_expression_call_effect_nonlocal_bindings(
                    object,
                    current_function_name,
                    names,
                    visited,
                );
                self.collect_expression_call_effect_nonlocal_bindings(
                    property,
                    current_function_name,
                    names,
                    visited,
                );
                self.collect_expression_call_effect_nonlocal_bindings(
                    value,
                    current_function_name,
                    names,
                    visited,
                );
            }
            Expression::AssignSuperMember { property, value } => {
                if let Some(effective_property) = self.resolve_property_key_expression(property) {
                    if let Some((_, binding)) = self
                        .resolve_super_runtime_prototype_binding_with_context(current_function_name)
                    {
                        if let Some(variants) =
                            self.resolve_user_super_setter_variants(&binding, &effective_property)
                        {
                            for (user_function, _) in variants {
                                names.extend(
                                    self.collect_user_function_call_effect_nonlocal_bindings_for_name(
                                        &user_function.name,
                                        visited,
                                    ),
                                );
                            }
                        }
                    } else if let Some(super_base) =
                        self.resolve_super_base_expression_with_context(current_function_name)
                        && let Some(LocalFunctionBinding::User(function_name)) =
                            self.resolve_member_setter_binding(&super_base, &effective_property)
                    {
                        names.extend(
                            self.collect_user_function_call_effect_nonlocal_bindings_for_name(
                                &function_name,
                                visited,
                            ),
                        );
                    }
                }
                self.collect_expression_call_effect_nonlocal_bindings(
                    property,
                    current_function_name,
                    names,
                    visited,
                );
                self.collect_expression_call_effect_nonlocal_bindings(
                    value,
                    current_function_name,
                    names,
                    visited,
                );
            }
            Expression::Member { object, property } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    object,
                    current_function_name,
                    names,
                    visited,
                );
                self.collect_expression_call_effect_nonlocal_bindings(
                    property,
                    current_function_name,
                    names,
                    visited,
                );
            }
            Expression::SuperMember { property } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    property,
                    current_function_name,
                    names,
                    visited,
                );
            }
            Expression::IteratorClose(value) => {
                let return_property = Expression::String("return".to_string());
                if let Some(LocalFunctionBinding::User(function_name)) = self
                    .resolve_member_function_binding(value, &return_property)
                    .or_else(|| {
                        let Expression::Identifier(iterator_name) = value.as_ref() else {
                            return None;
                        };
                        self.resolve_iterator_close_return_binding_in_function(
                            iterator_name,
                            current_function_name,
                        )
                    })
                {
                    names.extend(
                        self.collect_user_function_call_effect_nonlocal_bindings_for_name(
                            &function_name,
                            visited,
                        ),
                    );
                }
                self.collect_expression_call_effect_nonlocal_bindings(
                    value,
                    current_function_name,
                    names,
                    visited,
                );
            }
            Expression::Assign { value, .. }
            | Expression::Await(value)
            | Expression::EnumerateKeys(value)
            | Expression::GetIterator(value)
            | Expression::Unary {
                expression: value, ..
            } => self.collect_expression_call_effect_nonlocal_bindings(
                value,
                current_function_name,
                names,
                visited,
            ),
            Expression::Binary { left, right, .. } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    left,
                    current_function_name,
                    names,
                    visited,
                );
                self.collect_expression_call_effect_nonlocal_bindings(
                    right,
                    current_function_name,
                    names,
                    visited,
                );
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    condition,
                    current_function_name,
                    names,
                    visited,
                );
                self.collect_expression_call_effect_nonlocal_bindings(
                    then_expression,
                    current_function_name,
                    names,
                    visited,
                );
                self.collect_expression_call_effect_nonlocal_bindings(
                    else_expression,
                    current_function_name,
                    names,
                    visited,
                );
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.collect_expression_call_effect_nonlocal_bindings(
                        expression,
                        current_function_name,
                        names,
                        visited,
                    );
                }
            }
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                            self.collect_expression_call_effect_nonlocal_bindings(
                                expression,
                                current_function_name,
                                names,
                                visited,
                            );
                        }
                    }
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        ObjectEntry::Data { key, value } => {
                            self.collect_expression_call_effect_nonlocal_bindings(
                                key,
                                current_function_name,
                                names,
                                visited,
                            );
                            self.collect_expression_call_effect_nonlocal_bindings(
                                value,
                                current_function_name,
                                names,
                                visited,
                            );
                        }
                        ObjectEntry::Getter { key, .. } | ObjectEntry::Setter { key, .. } => {
                            self.collect_expression_call_effect_nonlocal_bindings(
                                key,
                                current_function_name,
                                names,
                                visited,
                            );
                        }
                        ObjectEntry::Spread(expression) => {
                            self.collect_expression_call_effect_nonlocal_bindings(
                                expression,
                                current_function_name,
                                names,
                                visited,
                            );
                        }
                    }
                }
            }
            Expression::Identifier(_)
            | Expression::Update { .. }
            | Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::NewTarget
            | Expression::Sent
            | Expression::This => {}
        }
    }

    pub(in crate::backend::direct_wasm) fn collect_user_function_argument_call_effect_nonlocal_bindings(
        &self,
        user_function: &UserFunction,
        arguments: &[Expression],
    ) -> HashSet<String> {
        let Some(function) = self.resolve_registered_function_declaration(&user_function.name)
        else {
            return HashSet::new();
        };
        let call_arguments = arguments
            .iter()
            .cloned()
            .map(CallArgument::Expression)
            .collect::<Vec<_>>();
        let mut iterator_names = Vec::new();
        Self::collect_iterator_close_binding_names_from_statements(
            &function.body,
            &mut iterator_names,
        );
        let mut names = HashSet::new();
        let mut visited = HashSet::new();
        for iterator_name in iterator_names {
            let Some(iterated) =
                Self::find_iterator_source_expression_in_statements(&function.body, &iterator_name)
            else {
                continue;
            };
            let iterated = self.substitute_user_function_argument_bindings(
                &iterated,
                user_function,
                &call_arguments,
            );
            let iterator_call = Expression::Call {
                callee: Box::new(Expression::Member {
                    object: Box::new(iterated),
                    property: Box::new(symbol_iterator_expression()),
                }),
                arguments: Vec::new(),
            };
            let Some(LocalFunctionBinding::User(function_name)) = self
                .inherited_member_function_bindings(&iterator_call)
                .into_iter()
                .find(|binding| binding.property == "return")
                .map(|binding| binding.binding)
            else {
                continue;
            };
            names.extend(
                self.collect_user_function_call_effect_nonlocal_bindings_for_name(
                    &function_name,
                    &mut visited,
                ),
            );
        }
        names
    }

    pub(in crate::backend::direct_wasm) fn invalidate_user_function_assigned_nonlocal_bindings(
        &mut self,
        user_function: &UserFunction,
    ) {
        let names = self.collect_user_function_call_effect_nonlocal_bindings(user_function);
        if !names.is_empty() {
            self.invalidate_static_binding_metadata_for_names(&names);
        }
    }

    pub(in crate::backend::direct_wasm) fn synced_prepared_user_function_capture_source_bindings(
        &self,
        prepared: &[PreparedCaptureBinding],
    ) -> HashSet<String> {
        prepared
            .iter()
            .filter_map(|binding| {
                self.user_function_capture_source_is_locally_bound(&binding.source_name)
                    .then_some(binding.source_name.clone())
            })
            .collect()
    }

    pub(in crate::backend::direct_wasm) fn synced_prepared_bound_user_function_capture_source_bindings(
        &self,
        prepared: &[PreparedBoundCaptureBinding],
    ) -> HashSet<String> {
        prepared
            .iter()
            .filter_map(|binding| binding.source_binding_name.clone())
            .collect()
    }

    pub(in crate::backend::direct_wasm) fn snapshot_user_function_capture_source_bindings(
        &self,
        prepared: &[PreparedCaptureBinding],
    ) -> HashMap<String, Expression> {
        prepared
            .iter()
            .filter(|binding| {
                self.user_function_capture_source_is_locally_bound(&binding.source_name)
            })
            .map(|binding| {
                (
                    binding.source_name.clone(),
                    self.snapshot_bound_capture_slot_expression(&binding.source_name),
                )
            })
            .collect()
    }

    pub(in crate::backend::direct_wasm) fn invalidate_user_function_call_effect_nonlocal_bindings_except(
        &mut self,
        user_function: &UserFunction,
        preserved_names: &HashSet<String>,
    ) {
        let names = self
            .collect_user_function_call_effect_nonlocal_bindings(user_function)
            .difference(preserved_names)
            .cloned()
            .collect::<HashSet<_>>();
        if !names.is_empty() {
            let preserved_kinds = names
                .iter()
                .filter_map(|name| {
                    self.lookup_identifier_kind(name)
                        .map(|kind| (name.clone(), kind))
                })
                .collect::<HashMap<_, _>>();
            self.invalidate_static_binding_metadata_for_names_with_preserved_kinds(
                &names,
                &preserved_kinds,
            );
        }
    }

    pub(in crate::backend::direct_wasm) fn sync_snapshot_user_function_call_effect_bindings(
        &mut self,
        names: &HashSet<String>,
        updated_bindings: Option<&HashMap<String, Expression>>,
        fallback_bindings: Option<&HashMap<String, Expression>>,
    ) -> DirectResult<HashSet<String>> {
        let mut unresolved = HashSet::new();
        for name in names {
            let Some(value) = updated_bindings
                .and_then(|bindings| bindings.get(name))
                .or_else(|| fallback_bindings.and_then(|bindings| bindings.get(name)))
            else {
                unresolved.insert(name.clone());
                continue;
            };
            self.sync_bound_capture_source_binding_metadata(name, value)?;
            self.runtime_dynamic_bindings.remove(name);
        }
        Ok(unresolved)
    }
}
