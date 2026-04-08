use super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct ArgumentsValueBinding {
    pub(in crate::backend::direct_wasm) values: Vec<Expression>,
    pub(in crate::backend::direct_wasm) strict: bool,
    pub(in crate::backend::direct_wasm) callee_present: bool,
    pub(in crate::backend::direct_wasm) callee_value: Option<Expression>,
    pub(in crate::backend::direct_wasm) length_present: bool,
    pub(in crate::backend::direct_wasm) length_value: Expression,
}

impl ArgumentsValueBinding {
    pub(in crate::backend::direct_wasm) fn for_user_function(
        user_function: &UserFunction,
        values: Vec<Expression>,
    ) -> Self {
        let mut binding = Self {
            length_value: Expression::Number(values.len() as f64),
            values,
            strict: user_function.strict,
            callee_present: true,
            callee_value: if user_function.strict {
                None
            } else {
                Some(Expression::Identifier(user_function.name.clone()))
            },
            length_present: true,
        };
        binding.apply_effects(&user_function.returned_arguments_effects);
        binding
    }

    pub(in crate::backend::direct_wasm) fn apply_effects(
        &mut self,
        effects: &ReturnedArgumentsEffects,
    ) {
        if let Some(effect) = &effects.callee {
            self.apply_named_effect("callee", effect.clone());
        }
        if let Some(effect) = &effects.length {
            self.apply_named_effect("length", effect.clone());
        }
    }

    pub(in crate::backend::direct_wasm) fn apply_named_effect(
        &mut self,
        property_name: &str,
        effect: ArgumentsPropertyEffect,
    ) {
        match property_name {
            "callee" => {
                if self.strict {
                    return;
                }
                match effect {
                    ArgumentsPropertyEffect::Assign(value) => {
                        self.callee_present = true;
                        self.callee_value = Some(value);
                    }
                    ArgumentsPropertyEffect::Delete => {
                        self.callee_present = false;
                        self.callee_value = None;
                    }
                }
            }
            "length" => match effect {
                ArgumentsPropertyEffect::Assign(value) => {
                    self.length_present = true;
                    self.length_value = value;
                }
                ArgumentsPropertyEffect::Delete => {
                    self.length_present = false;
                    self.length_value = Expression::Undefined;
                }
            },
            _ => {}
        }
    }
}
