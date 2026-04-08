use super::super::super::*;

impl GlobalMemberService {
    pub(in crate::backend::direct_wasm) fn reset_for_program(&mut self) {
        self.member_function_bindings.clear();
        self.member_function_capture_slots.clear();
        self.member_getter_bindings.clear();
        self.member_setter_bindings.clear();
    }
}
