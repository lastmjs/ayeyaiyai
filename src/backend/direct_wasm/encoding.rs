use super::*;

pub(super) fn encode_type_section(user_type_arities: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::new();
    push_u32(
        &mut bytes,
        USER_TYPE_BASE_INDEX + user_type_arities.len() as u32,
    );

    push_function_type(
        &mut bytes,
        &[I32_TYPE, I32_TYPE, I32_TYPE, I32_TYPE],
        &[I32_TYPE],
    );
    push_function_type(&mut bytes, &[I32_TYPE, I32_TYPE], &[]);
    push_function_type(&mut bytes, &[I32_TYPE], &[]);
    push_function_type(&mut bytes, &[], &[]);
    for arity in user_type_arities {
        let params = vec![I32_TYPE; *arity as usize];
        push_function_type(&mut bytes, &params, &[I32_TYPE]);
    }

    bytes
}

pub(super) fn encode_import_section() -> Vec<u8> {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 1);
    push_name(&mut bytes, "wasi_snapshot_preview1");
    push_name(&mut bytes, "fd_write");
    bytes.push(0x00);
    push_u32(&mut bytes, WASI_FD_WRITE_TYPE_INDEX);
    bytes
}

pub(super) fn encode_function_section(user_functions: &[UserFunction]) -> Vec<u8> {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 5 + user_functions.len() as u32);
    push_u32(&mut bytes, WRITE_BYTES_TYPE_INDEX);
    push_u32(&mut bytes, UNARY_VOID_TYPE_INDEX);
    push_u32(&mut bytes, UNARY_VOID_TYPE_INDEX);
    push_u32(&mut bytes, UNARY_VOID_TYPE_INDEX);
    push_u32(&mut bytes, START_TYPE_INDEX);
    for function in user_functions {
        push_u32(&mut bytes, function.type_index);
    }
    bytes
}

pub(in crate::backend::direct_wasm) fn required_memory_pages(data_end_offset: u32) -> u32 {
    data_end_offset.max(1).div_ceil(WASM_MEMORY_PAGE_SIZE)
}

pub(super) fn encode_memory_section(initial_pages: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 1);
    bytes.push(0x00);
    push_u32(&mut bytes, initial_pages.max(1));
    bytes
}

pub(super) fn encode_global_section(global_binding_count: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 4 + global_binding_count);

    for initial_value in [0, 0, JS_UNDEFINED_TAG, JS_TYPEOF_OBJECT_TAG]
        .into_iter()
        .chain(std::iter::repeat(0).take(global_binding_count as usize))
    {
        bytes.push(I32_TYPE);
        bytes.push(0x01);
        bytes.push(0x41);
        push_i32(&mut bytes, initial_value);
        bytes.push(0x0b);
    }

    bytes
}

pub(super) fn encode_export_section() -> Vec<u8> {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 2);

    push_name(&mut bytes, "_start");
    bytes.push(0x00);
    push_u32(&mut bytes, START_FUNCTION_INDEX);

    push_name(&mut bytes, "memory");
    bytes.push(0x02);
    push_u32(&mut bytes, 0);

    bytes
}

pub(super) fn encode_code_section(
    start: CompiledFunction,
    user_functions: Vec<CompiledFunction>,
    int_min_ptr: u32,
    int_min_len: u32,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 5 + user_functions.len() as u32);
    push_function_body(&mut bytes, &[], &write_bytes_body());
    push_function_body(&mut bytes, &[], &write_char_body());
    push_function_body(&mut bytes, &[], &print_u32_body());
    push_function_body(&mut bytes, &[], &print_i32_body(int_min_ptr, int_min_len));
    push_function_body(
        &mut bytes,
        &[(start.local_count, I32_TYPE)],
        &start.instructions,
    );
    for function in user_functions {
        push_function_body(
            &mut bytes,
            &[(function.local_count, I32_TYPE)],
            &function.instructions,
        );
    }
    bytes
}

pub(super) fn encode_data_section(segments: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, segments.len() as u32);

    for (offset, data) in segments {
        bytes.push(0x00);
        bytes.push(0x41);
        push_i32(&mut bytes, *offset as i32);
        bytes.push(0x0b);
        push_bytes(&mut bytes, data);
    }

    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_required_memory_pages_from_static_data_end() {
        assert_eq!(required_memory_pages(0), 1);
        assert_eq!(required_memory_pages(DATA_START_OFFSET), 1);
        assert_eq!(required_memory_pages(WASM_MEMORY_PAGE_SIZE), 1);
        assert_eq!(required_memory_pages(WASM_MEMORY_PAGE_SIZE + 1), 2);
        assert_eq!(required_memory_pages(WASM_MEMORY_PAGE_SIZE * 2), 2);
    }
}

pub(super) fn push_section(module: &mut Vec<u8>, section_id: u8, contents: Vec<u8>) {
    module.push(section_id);
    push_u32(module, contents.len() as u32);
    module.extend_from_slice(&contents);
}

pub(super) fn push_u32(bytes: &mut Vec<u8>, mut value: u32) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        bytes.push(byte);
        if value == 0 {
            break;
        }
    }
}

pub(super) fn push_i32(bytes: &mut Vec<u8>, mut value: i32) {
    loop {
        let byte = (value as u8) & 0x7f;
        value >>= 7;
        let done = (value == 0 && (byte & 0x40) == 0) || (value == -1 && (byte & 0x40) != 0);
        bytes.push(if done { byte } else { byte | 0x80 });
        if done {
            break;
        }
    }
}

fn write_bytes_body() -> Vec<u8> {
    let mut body = Vec::new();

    body.push(0x41);
    push_i32(&mut body, IOVEC_OFFSET as i32);
    body.push(0x20);
    push_u32(&mut body, 0);
    body.push(0x36);
    body.push(0x02);
    push_u32(&mut body, 0);

    body.push(0x41);
    push_i32(&mut body, (IOVEC_OFFSET + 4) as i32);
    body.push(0x20);
    push_u32(&mut body, 1);
    body.push(0x36);
    body.push(0x02);
    push_u32(&mut body, 0);

    body.push(0x41);
    push_i32(&mut body, 1);
    body.push(0x41);
    push_i32(&mut body, IOVEC_OFFSET as i32);
    body.push(0x41);
    push_i32(&mut body, 1);
    body.push(0x41);
    push_i32(&mut body, NWRITTEN_OFFSET as i32);
    body.push(0x10);
    push_u32(&mut body, FD_WRITE_FUNCTION_INDEX);
    body.push(0x1a);

    body
}

fn write_char_body() -> Vec<u8> {
    let mut body = Vec::new();

    body.push(0x41);
    push_i32(&mut body, CHAR_OFFSET as i32);
    body.push(0x20);
    push_u32(&mut body, 0);
    body.push(0x3a);
    body.push(0x00);
    push_u32(&mut body, 0);

    body.push(0x41);
    push_i32(&mut body, CHAR_OFFSET as i32);
    body.push(0x41);
    push_i32(&mut body, 1);
    body.push(0x10);
    push_u32(&mut body, WRITE_BYTES_FUNCTION_INDEX);

    body
}

fn print_u32_body() -> Vec<u8> {
    let mut body = Vec::new();

    body.push(0x20);
    push_u32(&mut body, 0);
    body.push(0x41);
    push_i32(&mut body, 10);
    body.push(0x4f);
    body.push(0x04);
    body.push(EMPTY_BLOCK_TYPE);
    body.push(0x20);
    push_u32(&mut body, 0);
    body.push(0x41);
    push_i32(&mut body, 10);
    body.push(0x6e);
    body.push(0x10);
    push_u32(&mut body, PRINT_U32_FUNCTION_INDEX);
    body.push(0x0b);

    body.push(0x41);
    push_i32(&mut body, 48);
    body.push(0x20);
    push_u32(&mut body, 0);
    body.push(0x41);
    push_i32(&mut body, 10);
    body.push(0x70);
    body.push(0x6a);
    body.push(0x10);
    push_u32(&mut body, WRITE_CHAR_FUNCTION_INDEX);

    body
}

fn print_i32_body(int_min_ptr: u32, int_min_len: u32) -> Vec<u8> {
    let mut body = Vec::new();

    body.push(0x20);
    push_u32(&mut body, 0);
    body.push(0x41);
    push_i32(&mut body, 0);
    body.push(0x48);
    body.push(0x04);
    body.push(EMPTY_BLOCK_TYPE);

    body.push(0x20);
    push_u32(&mut body, 0);
    body.push(0x41);
    push_i32(&mut body, i32::MIN);
    body.push(0x46);
    body.push(0x04);
    body.push(EMPTY_BLOCK_TYPE);
    body.push(0x41);
    push_i32(&mut body, int_min_ptr as i32);
    body.push(0x41);
    push_i32(&mut body, int_min_len as i32);
    body.push(0x10);
    push_u32(&mut body, WRITE_BYTES_FUNCTION_INDEX);
    body.push(0x0f);
    body.push(0x0b);

    body.push(0x41);
    push_i32(&mut body, b'-' as i32);
    body.push(0x10);
    push_u32(&mut body, WRITE_CHAR_FUNCTION_INDEX);
    body.push(0x41);
    push_i32(&mut body, 0);
    body.push(0x20);
    push_u32(&mut body, 0);
    body.push(0x6b);
    body.push(0x10);
    push_u32(&mut body, PRINT_U32_FUNCTION_INDEX);
    body.push(0x0f);
    body.push(0x0b);

    body.push(0x20);
    push_u32(&mut body, 0);
    body.push(0x10);
    push_u32(&mut body, PRINT_U32_FUNCTION_INDEX);

    body
}

fn push_function_type(bytes: &mut Vec<u8>, params: &[u8], results: &[u8]) {
    bytes.push(0x60);
    push_u32(bytes, params.len() as u32);
    bytes.extend_from_slice(params);
    push_u32(bytes, results.len() as u32);
    bytes.extend_from_slice(results);
}

fn push_function_body(bytes: &mut Vec<u8>, locals: &[(u32, u8)], instructions: &[u8]) {
    let mut body = Vec::new();
    let local_groups = locals.iter().filter(|(count, _)| *count > 0).count() as u32;
    push_u32(&mut body, local_groups);
    for (count, value_type) in locals.iter().filter(|(count, _)| *count > 0) {
        push_u32(&mut body, *count);
        body.push(*value_type);
    }
    body.extend_from_slice(instructions);
    body.push(0x0b);

    push_u32(bytes, body.len() as u32);
    bytes.extend_from_slice(&body);
}

fn push_name(bytes: &mut Vec<u8>, name: &str) {
    push_bytes(bytes, name.as_bytes());
}

fn push_bytes(bytes: &mut Vec<u8>, data: &[u8]) {
    push_u32(bytes, data.len() as u32);
    bytes.extend_from_slice(data);
}
