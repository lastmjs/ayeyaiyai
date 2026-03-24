use std::collections::HashSet;

#[derive(Default)]
pub(super) struct ScopeStack {
    scopes: Vec<HashSet<String>>,
}

impl ScopeStack {
    pub(super) fn push(&mut self, scope: HashSet<String>) {
        self.scopes.push(scope);
    }

    pub(super) fn pop(&mut self) -> Option<HashSet<String>> {
        self.scopes.pop()
    }

    pub(super) fn contains(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|scope| scope.contains(name))
    }
}
