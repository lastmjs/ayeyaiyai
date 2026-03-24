use super::super::*;

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

pub(crate) fn asyncify_statements(statements: Vec<Statement>) -> (Vec<Statement>, bool) {
    let mut asyncified = Vec::new();
    let mut changed = false;

    for statement in statements {
        let (mut lowered, statement_changed) = asyncify_statement(statement);
        changed |= statement_changed;
        asyncified.append(&mut lowered);
    }

    (asyncified, changed)
}
