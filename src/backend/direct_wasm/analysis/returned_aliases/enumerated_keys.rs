use super::*;

pub(in crate::backend::direct_wasm) fn collect_enumerated_keys_param_index(
    function: &FunctionDeclaration,
) -> Option<usize> {
    let returned_identifier = collect_returned_identifier(&function.body)?;
    let initialized_array = function.body.iter().any(|statement| {
        matches!(
            statement,
            Statement::Var { name, value }
                | Statement::Let { name, value, .. }
                | Statement::Assign { name, value }
                if name == &returned_identifier
                    && matches!(value, Expression::Array(elements) if elements.is_empty())
        )
    });
    if !initialized_array {
        return None;
    }

    function.body.iter().find_map(|statement| {
        match_enumerated_keys_collector_loop(statement, &returned_identifier, function)
    })
}

pub(in crate::backend::direct_wasm) fn match_enumerated_keys_collector_loop(
    statement: &Statement,
    returned_identifier: &str,
    function: &FunctionDeclaration,
) -> Option<usize> {
    let Statement::For {
        init,
        condition,
        update,
        body,
        ..
    } = statement
    else {
        return None;
    };

    let (target_name, param_index) = init.iter().find_map(|statement| match statement {
        Statement::Let { name, value, .. } | Statement::Var { name, value } => {
            let Expression::Identifier(param_name) = value else {
                return None;
            };
            function
                .params
                .iter()
                .position(|parameter| parameter.name == *param_name)
                .map(|param_index| (name.clone(), param_index))
        }
        _ => None,
    })?;

    let keys_name = init.iter().find_map(|statement| match statement {
        Statement::Let { name, value, .. } | Statement::Var { name, value } => {
            let Expression::EnumerateKeys(target) = value else {
                return None;
            };
            matches!(target.as_ref(), Expression::Identifier(current_target) if current_target == &target_name)
                .then(|| name.clone())
        }
        _ => None,
    })?;

    let index_name = match condition.as_ref()? {
        Expression::Binary {
            op: BinaryOp::LessThan,
            left,
            right,
        } => {
            let Expression::Identifier(index_name) = left.as_ref() else {
                return None;
            };
            matches!(
                right.as_ref(),
                Expression::Member { object, property }
                    if matches!(object.as_ref(), Expression::Identifier(name) if name == &keys_name)
                        && matches!(property.as_ref(), Expression::String(property_name) if property_name == "length")
            )
            .then(|| index_name.clone())?
        }
        _ => return None,
    };

    if !matches!(
        update.as_ref()?,
        Expression::Update {
            name,
            op: UpdateOp::Increment,
            ..
        } if name == &index_name
    ) {
        return None;
    }

    let loop_value_name = body.iter().find_map(|statement| match statement {
        Statement::Let { name, value, .. } | Statement::Var { name, value } => matches!(
            value,
            Expression::Member { object, property }
                if matches!(object.as_ref(), Expression::Identifier(current_keys) if current_keys == &keys_name)
                    && matches!(property.as_ref(), Expression::Identifier(current_index) if current_index == &index_name)
        )
        .then(|| name.clone()),
        _ => None,
    })?;

    body.iter().any(|statement| {
        matches!(
            statement,
            Statement::Expression(Expression::Call { callee, arguments })
                if matches!(
                    callee.as_ref(),
                    Expression::Member { object, property }
                        if matches!(object.as_ref(), Expression::Identifier(name) if name == returned_identifier)
                            && matches!(property.as_ref(), Expression::String(property_name) if property_name == "push")
                ) && matches!(
                    arguments.as_slice(),
                    [CallArgument::Expression(Expression::Identifier(argument_name))]
                        if argument_name == &loop_value_name
                )
        )
    })
    .then_some(param_index)
}
