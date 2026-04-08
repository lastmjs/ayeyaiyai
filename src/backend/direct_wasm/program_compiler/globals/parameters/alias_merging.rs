use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn merge_aliases_for_branches(
        &self,
        baseline: &HashMap<String, Option<LocalFunctionBinding>>,
        branches: &[&HashMap<String, Option<LocalFunctionBinding>>],
    ) -> HashMap<String, Option<LocalFunctionBinding>> {
        let mut merged = baseline.clone();
        for (name, baseline_binding) in baseline {
            for branch in branches {
                if branch.get(name) != Some(baseline_binding) {
                    merged.insert(name.clone(), None);
                    break;
                }
            }
        }
        merged
    }

    pub(in crate::backend::direct_wasm) fn merge_aliases_for_optional_body(
        &self,
        before_body: &HashMap<String, Option<LocalFunctionBinding>>,
        after_body: &HashMap<String, Option<LocalFunctionBinding>>,
    ) -> HashMap<String, Option<LocalFunctionBinding>> {
        self.merge_aliases_for_branches(before_body, &[before_body, after_body])
    }
}
