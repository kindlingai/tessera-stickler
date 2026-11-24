//! Line shapes and merge conflicts, for tools/lint and tools/simcheck.

use std::iter::Peekable;
use std::str::Chars;

fn eat(chars: &mut Peekable<Chars>, keep: impl Fn(char) -> bool) {
    while chars.next_if(|&d| keep(d)).is_some() {}
}

/// A line with identifiers, numbers and strings collapsed.
pub fn shape(line: &str) -> String {
    let mut out = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_alphabetic() || c == '_' {
            eat(&mut chars, |d| d.is_alphanumeric() || d == '_');
            out.push('a');
        } else if c.is_ascii_digit() {
            eat(&mut chars, |d| d.is_alphanumeric() || d == '.');
            out.push('0');
        } else if c == '"' {
            let mut escaped = false;
            for d in chars.by_ref() {
                match (escaped, d) {
                    (true, _) => escaped = false,
                    (false, '\\') => escaped = true,
                    (false, '"') => break,
                    _ => {}
                }
            }
            out.push('"');
        } else if !c.is_whitespace() {
            out.push(c);
        }
    }
    out
}

/// The first line, from 1, of an unresolved merge conflict.
pub fn conflict(text: &str) -> Option<usize> {
    let marker = |l: &str| l == "<<<<<<<" || l.starts_with("<<<<<<< ");
    text.lines().position(marker).map(|i| i + 1)
}
