use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn allocate_parameter_support_locals(
        parameter_names: &[String],
        has_parameter_defaults: bool,
        needs_parameter_scope_arguments_local: bool,
        next_local_index: &mut u32,
    ) -> (Option<u32>, HashMap<String, u32>) {
        let parameter_scope_arguments_local = if needs_parameter_scope_arguments_local {
            let local_index = *next_local_index;
            *next_local_index += 1;
            Some(local_index)
        } else {
            None
        };
        let mut parameter_initialized_locals = HashMap::new();
        if has_parameter_defaults {
            for param in parameter_names {
                if parameter_initialized_locals.contains_key(param) {
                    continue;
                }
                parameter_initialized_locals.insert(param.clone(), *next_local_index);
                *next_local_index += 1;
            }
        }
        (
            parameter_scope_arguments_local,
            parameter_initialized_locals,
        )
    }
}
