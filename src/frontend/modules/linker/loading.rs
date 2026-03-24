use super::super::*;

impl ModuleLinker {
    pub(crate) fn ensure_module_slot(&mut self, path: &Path) -> Result<usize> {
        let resolved = normalize_module_path(path)?;
        if let Some(index) = self.module_indices.get(&resolved).copied() {
            return Ok(index);
        }

        let module_index = self.modules.len();
        self.module_indices.insert(resolved.clone(), module_index);
        self.modules.push(LinkedModule {
            path: resolved.clone(),
            state: ModuleState::Reserved,
            namespace_name: format!("__ayy_module_namespace_{module_index}"),
            init_name: format!("__ayy_module_init_{module_index}"),
            promise_name: format!("__ayy_module_promise_{module_index}"),
            init_async: false,
            dependency_params: Vec::new(),
            export_names: Vec::new(),
            export_resolutions: BTreeMap::new(),
            ambiguous_export_names: HashSet::new(),
        });

        Ok(module_index)
    }

    pub(crate) fn load_module(&mut self, path: &Path) -> Result<usize> {
        let module_index = self.ensure_module_slot(path)?;
        if self.modules[module_index].state != ModuleState::Reserved {
            return Ok(module_index);
        }

        let resolved = self.modules[module_index].path.clone();
        let (module, source_text) = parse_module_file(&resolved)?;
        self.modules[module_index].state = ModuleState::Lowering;
        self.predeclare_module_export_resolutions(module_index, &module, &resolved)?;
        self.lower_module(module_index, &module, source_text)?;
        self.modules[module_index].state = ModuleState::Lowered;

        Ok(module_index)
    }

    pub(crate) fn compute_static_load_order(&self, entry_index: usize) -> Vec<usize> {
        fn visit(
            linker: &ModuleLinker,
            module_index: usize,
            visited: &mut HashSet<usize>,
            order: &mut Vec<usize>,
        ) {
            if !visited.insert(module_index) {
                return;
            }

            for dependency in &linker.modules[module_index].dependency_params {
                if dependency.module_index != module_index {
                    visit(linker, dependency.module_index, visited, order);
                }
            }

            order.push(module_index);
        }

        let mut order = Vec::new();
        visit(self, entry_index, &mut HashSet::new(), &mut order);
        order
    }
}
