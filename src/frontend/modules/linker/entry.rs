use super::super::*;

impl ModuleLinker {
    pub(crate) fn bundle_entry(&mut self, path: &Path) -> Result<Program> {
        let entry_index = self.load_module(path)?;
        self.load_order = self.compute_static_load_order(entry_index);
        let statements = self.bundle_statements(entry_index)?;
        Ok(self.lowerer.finish_program(statements, true))
    }

    pub(crate) fn bundle_script_entry(&mut self, path: &Path) -> Result<Program> {
        let (script, lowered_source) = parse_script_file(path)?;
        for source in collect_literal_dynamic_import_specifiers_in_statements(&script.body) {
            if let Ok(dependency_path) = resolve_module_specifier(path, &source) {
                self.load_module(&dependency_path)?;
            }
        }

        self.lowerer.source_text = Some(lowered_source);
        self.lowerer.current_module_path = Some(normalize_module_path(path)?);
        self.lowerer.module_index_lookup = self.module_indices.clone();
        let strict = script_has_use_strict_directive(&script.body);
        self.lowerer.strict_modes.push(strict);
        self.lowerer.module_mode = false;

        let mut statements = self.module_registry_statements();
        let scope_bindings = collect_direct_statement_lexical_bindings(&script.body)?;
        self.lowerer.push_binding_scope(scope_bindings);
        let lowered = self
            .lowerer
            .lower_top_level_statements(script.body.iter(), &mut statements);
        self.lowerer.pop_binding_scope();
        lowered?;

        self.lowerer.strict_modes.pop();
        self.lowerer.module_mode = false;
        self.lowerer.source_text = None;
        self.lowerer.current_module_path = None;
        self.lowerer.module_index_lookup.clear();

        Ok(self.lowerer.finish_program(statements, strict))
    }
}
