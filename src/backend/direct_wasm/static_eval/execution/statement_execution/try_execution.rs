use super::super::*;
use super::{StaticStatementExecutor, execute_static_statement_block};

pub(in crate::backend::direct_wasm) fn execute_static_try_block<Executor, Environment>(
    executor: &Executor,
    body: &[Statement],
    catch_setup: &[Statement],
    catch_body: &[Statement],
    environment: &mut Environment,
) -> Option<StaticStatementControl>
where
    Executor: StaticStatementExecutor<Environment = Environment> + ?Sized,
    Environment: StaticTransactionalEnvironment,
{
    let mut try_environment = environment.fork_environment();
    match execute_static_statement_block(executor, body, &mut try_environment) {
        Some(StaticStatementControl::Return(result)) => {
            Some(StaticStatementControl::Return(result))
        }
        Some(StaticStatementControl::Continue) => {
            environment.commit_environment(try_environment);
            Some(StaticStatementControl::Continue)
        }
        None => {
            if let StaticStatementControl::Return(result) =
                execute_static_statement_block(executor, catch_setup, &mut try_environment)?
            {
                return Some(StaticStatementControl::Return(result));
            }
            match execute_static_statement_block(executor, catch_body, &mut try_environment)? {
                StaticStatementControl::Continue => {
                    environment.commit_environment(try_environment);
                    Some(StaticStatementControl::Continue)
                }
                StaticStatementControl::Return(result) => {
                    Some(StaticStatementControl::Return(result))
                }
            }
        }
    }
}

pub(in crate::backend::direct_wasm) fn execute_static_try_statement<
    Executor: StaticStatementExecutor<Environment = Environment> + ?Sized,
    Environment: StaticTransactionalEnvironment,
>(
    executor: &Executor,
    body: &[Statement],
    catch_setup: &[Statement],
    catch_body: &[Statement],
    environment: &mut Environment,
) -> Option<StaticStatementControl> {
    execute_static_try_block(executor, body, catch_setup, catch_body, environment)
}
