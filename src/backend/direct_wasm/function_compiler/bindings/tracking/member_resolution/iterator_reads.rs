use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_direct_iterator_step_member_read(
        &mut self,
        object: &Expression,
        property: &Expression,
    ) -> DirectResult<bool> {
        let Expression::String(property_name) = property else {
            return Ok(false);
        };
        if property_name != "done" && property_name != "value" {
            return Ok(false);
        }
        let Expression::Call { callee, arguments } = object else {
            return Ok(false);
        };
        if !arguments.is_empty() {
            return Ok(false);
        }
        let Expression::Member {
            object: iterator_object,
            property: next_property,
        } = callee.as_ref()
        else {
            return Ok(false);
        };
        if !matches!(next_property.as_ref(), Expression::String(name) if name == "next") {
            return Ok(false);
        }
        let hidden_name =
            self.allocate_named_hidden_local("direct_iterator_step", StaticValueKind::Object);
        self.update_local_iterator_step_binding(&hidden_name, object);
        let Some(IteratorStepBinding::Runtime {
            done_local,
            value_local,
            ..
        }) = self
            .state
            .speculation
            .static_semantics
            .local_iterator_step_binding(&hidden_name)
            .cloned()
        else {
            return Ok(false);
        };
        self.emit_numeric_expression(iterator_object)?;
        self.state.emission.output.instructions.push(0x1a);
        match property_name.as_str() {
            "done" => self.push_local_get(done_local),
            "value" => self.push_local_get(value_local),
            _ => unreachable!("filtered above"),
        }
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn direct_iterator_binding_source_expression<'b>(
        &self,
        value: &'b Expression,
    ) -> Option<&'b Expression> {
        let iterated = match value {
            Expression::GetIterator(iterated) => iterated.as_ref(),
            Expression::Call { callee, arguments }
                if arguments.is_empty()
                    && matches!(
                        callee.as_ref(),
                        Expression::Member { property, .. }
                            if is_symbol_iterator_expression(property)
                    ) =>
            {
                let Expression::Member { object, .. } = callee.as_ref() else {
                    unreachable!("filtered above");
                };
                object.as_ref()
            }
            _ => return None,
        };
        let next_property = Expression::String("next".to_string());
        let has_next_binding = self
            .resolve_member_function_binding(iterated, &next_property)
            .is_some();
        let has_iterator_source_kind = self.resolve_iterator_source_kind(iterated).is_some();
        let has_next_property = self
            .resolve_object_binding_from_expression(iterated)
            .is_some_and(|object_binding| {
                object_binding_has_property(&object_binding, &next_property)
            });
        if has_next_binding || has_iterator_source_kind || has_next_property {
            return Some(iterated);
        }
        None
    }
}
