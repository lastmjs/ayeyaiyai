use super::*;

impl StaticFunctionExecutionEnvironment for GlobalStaticEvaluationEnvironment {
    fn clear_function_locals(&mut self) {
        self.clear_local_bindings();
    }
}

impl StaticBindingEnvironment for GlobalStaticEvaluationEnvironment {
    fn assign_binding_value(&mut self, name: String, value: Expression) -> Expression {
        GlobalStaticEvaluationEnvironment::assign_binding_value(self, name, value)
    }

    fn sync_object_binding(&mut self, name: &str, binding: Option<ObjectValueBinding>) {
        GlobalStaticEvaluationEnvironment::sync_object_binding(self, name, binding);
    }
}

impl StaticLocalBindingEnvironment for GlobalStaticEvaluationEnvironment {
    fn set_local_binding(&mut self, name: String, value: Expression) -> Expression {
        GlobalStaticEvaluationEnvironment::set_local_binding(self, name, value)
    }
}

impl StaticObjectBindingLookupEnvironment for GlobalStaticEvaluationEnvironment {
    fn binding(&self, name: &str) -> Option<&Expression> {
        self.local_bindings
            .get(name)
            .or_else(|| self.value_bindings.get(name))
    }

    fn contains_object_binding(&self, name: &str) -> bool {
        self.object_bindings.contains_key(name)
    }
}

impl StaticObjectBindingEnvironment for GlobalStaticEvaluationEnvironment {
    fn object_binding(&self, name: &str) -> Option<&ObjectValueBinding> {
        self.object_bindings.get(name)
    }
}

impl StaticMutableObjectBindingEnvironment for GlobalStaticEvaluationEnvironment {
    fn object_binding_mut(&mut self, name: &str) -> Option<&mut ObjectValueBinding> {
        GlobalStaticEvaluationEnvironment::object_binding_mut(self, name)
    }

    fn set_object_binding(&mut self, name: String, binding: ObjectValueBinding) {
        self.object_bindings.insert(name, binding);
    }
}

impl StaticIdentifierBindingEnvironment for GlobalStaticEvaluationEnvironment {
    fn local_binding(&self, name: &str) -> Option<&Expression> {
        self.local_bindings.get(name)
    }

    fn global_value_binding(&self, name: &str) -> Option<&Expression> {
        self.value_bindings.get(name)
    }
}

impl StaticTransactionalEnvironment for GlobalStaticEvaluationEnvironment {
    fn fork_environment(&self) -> Self {
        self.clone()
    }

    fn commit_environment(&mut self, environment: Self) {
        *self = environment;
    }
}

impl StaticFunctionExecutionEnvironment for StaticResolutionEnvironment {
    fn clear_function_locals(&mut self) {
        self.clear_local_bindings();
    }
}

impl StaticBindingEnvironment for StaticResolutionEnvironment {
    fn assign_binding_value(&mut self, name: String, value: Expression) -> Expression {
        StaticResolutionEnvironment::assign_binding_value(self, name, value)
    }

    fn sync_object_binding(&mut self, name: &str, binding: Option<ObjectValueBinding>) {
        StaticResolutionEnvironment::sync_object_binding(self, name, binding);
    }
}

impl StaticLocalBindingEnvironment for StaticResolutionEnvironment {
    fn set_local_binding(&mut self, name: String, value: Expression) -> Expression {
        StaticResolutionEnvironment::set_local_binding(self, name, value)
    }
}

impl StaticObjectBindingLookupEnvironment for StaticResolutionEnvironment {
    fn binding(&self, name: &str) -> Option<&Expression> {
        StaticResolutionEnvironment::binding(self, name)
    }

    fn contains_object_binding(&self, name: &str) -> bool {
        StaticResolutionEnvironment::contains_object_binding(self, name)
    }
}

impl StaticObjectBindingEnvironment for StaticResolutionEnvironment {
    fn object_binding(&self, name: &str) -> Option<&ObjectValueBinding> {
        StaticResolutionEnvironment::object_binding(self, name)
    }
}

impl StaticMutableObjectBindingEnvironment for StaticResolutionEnvironment {
    fn object_binding_mut(&mut self, name: &str) -> Option<&mut ObjectValueBinding> {
        StaticResolutionEnvironment::object_binding_mut(self, name)
    }

    fn set_object_binding(&mut self, name: String, binding: ObjectValueBinding) {
        StaticResolutionEnvironment::set_object_binding(self, name, binding);
    }
}

impl StaticIdentifierBindingEnvironment for StaticResolutionEnvironment {
    fn local_binding(&self, name: &str) -> Option<&Expression> {
        StaticResolutionEnvironment::local_binding(self, name)
    }

    fn global_value_binding(&self, name: &str) -> Option<&Expression> {
        StaticResolutionEnvironment::global_value_binding(self, name)
    }
}

impl StaticTransactionalEnvironment for StaticResolutionEnvironment {
    fn fork_environment(&self) -> Self {
        self.fork()
    }

    fn commit_environment(&mut self, environment: Self) {
        *self = environment;
    }
}
