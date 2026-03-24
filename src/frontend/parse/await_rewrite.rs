pub(super) fn rewrite_script_await_identifiers(source: &str) -> Option<String> {
    #[derive(Clone, Copy)]
    enum State {
        Code,
        SingleQuoted,
        DoubleQuoted,
        Template,
        LineComment,
        BlockComment,
    }

    fn is_ident_start(character: char) -> bool {
        character == '_' || character == '$' || character.is_ascii_alphabetic()
    }

    fn is_ident_continue(character: char) -> bool {
        is_ident_start(character) || character.is_ascii_digit()
    }

    fn starts_unicode_escape(characters: &[char], index: usize) -> bool {
        matches!(
            characters.get(index..index + 6),
            Some(['\\', 'u', a, b, c, d])
                if a.is_ascii_hexdigit()
                    && b.is_ascii_hexdigit()
                    && c.is_ascii_hexdigit()
                    && d.is_ascii_hexdigit()
        )
    }

    fn decode_unicode_escape(characters: &[char], index: usize) -> Option<(usize, char)> {
        let digits = characters.get(index + 2..index + 6)?;
        let hex = digits.iter().collect::<String>();
        let value = u32::from_str_radix(&hex, 16).ok()?;
        Some((index + 6, char::from_u32(value)?))
    }

    let characters = source.chars().collect::<Vec<_>>();
    let mut rewritten = String::with_capacity(source.len());
    let mut state = State::Code;
    let mut index = 0;
    let mut changed = false;

    while index < characters.len() {
        let character = characters[index];
        let next = characters.get(index + 1).copied();

        match state {
            State::Code => {
                if character == '\'' {
                    state = State::SingleQuoted;
                    rewritten.push(character);
                    index += 1;
                    continue;
                }
                if character == '"' {
                    state = State::DoubleQuoted;
                    rewritten.push(character);
                    index += 1;
                    continue;
                }
                if character == '`' {
                    state = State::Template;
                    rewritten.push(character);
                    index += 1;
                    continue;
                }
                if character == '/' && next == Some('/') {
                    state = State::LineComment;
                    rewritten.push('/');
                    rewritten.push('/');
                    index += 2;
                    continue;
                }
                if character == '/' && next == Some('*') {
                    state = State::BlockComment;
                    rewritten.push('/');
                    rewritten.push('*');
                    index += 2;
                    continue;
                }
                if is_ident_start(character) || starts_unicode_escape(&characters, index) {
                    let mut word = String::new();
                    while index < characters.len() {
                        if is_ident_continue(characters[index]) {
                            word.push(characters[index]);
                            index += 1;
                        } else if starts_unicode_escape(&characters, index) {
                            let Some((next_index, decoded)) =
                                decode_unicode_escape(&characters, index)
                            else {
                                break;
                            };
                            word.push(decoded);
                            index = next_index;
                        } else {
                            break;
                        }
                    }

                    if word == "await" {
                        rewritten.push_str("__ayy_await_ident");
                        changed = true;
                    } else {
                        rewritten.push_str(&word);
                    }
                    continue;
                }

                rewritten.push(character);
                index += 1;
            }
            State::SingleQuoted => {
                rewritten.push(character);
                index += 1;
                if character == '\\' && index < characters.len() {
                    rewritten.push(characters[index]);
                    index += 1;
                } else if character == '\'' {
                    state = State::Code;
                }
            }
            State::DoubleQuoted => {
                rewritten.push(character);
                index += 1;
                if character == '\\' && index < characters.len() {
                    rewritten.push(characters[index]);
                    index += 1;
                } else if character == '"' {
                    state = State::Code;
                }
            }
            State::Template => {
                rewritten.push(character);
                index += 1;
                if character == '\\' && index < characters.len() {
                    rewritten.push(characters[index]);
                    index += 1;
                } else if character == '`' {
                    state = State::Code;
                }
            }
            State::LineComment => {
                rewritten.push(character);
                index += 1;
                if character == '\n' || character == '\r' {
                    state = State::Code;
                }
            }
            State::BlockComment => {
                rewritten.push(character);
                index += 1;
                if character == '*' && next == Some('/') {
                    rewritten.push('/');
                    index += 1;
                    state = State::Code;
                }
            }
        }
    }

    changed.then_some(rewritten)
}
