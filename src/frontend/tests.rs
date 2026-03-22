use super::parse;

#[test]
fn parses_hashbang_comments_terminated_by_carriage_return() {
    parse("#! comment\r{}\n").expect("carriage-return-terminated hashbang should parse");
}

#[test]
fn parses_hashbang_comments_terminated_by_line_separator() {
    parse("#! comment\u{2028}{}\n").expect("line-separator-terminated hashbang should parse");
}

#[test]
fn parses_hashbang_comments_terminated_by_paragraph_separator() {
    parse("#! comment\u{2029}{}\n").expect("paragraph-separator-terminated hashbang should parse");
}
