//! Line shapes, merge conflicts and globs, for tools/lint and tools/simcheck.

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

/// A later `!` glob drops what earlier globs match.
pub fn matches(globs: &[String], path: &str) -> bool {
    let mut hit = false;
    for g in globs {
        match g.strip_prefix('!') {
            Some(g) => hit &= !glob(g, path),
            None => hit |= glob(g, path),
        }
    }
    hit
}

/// A trailing `/` matches files under the directory.
fn glob(pattern: &str, path: &str) -> bool {
    let (pattern, under) = match pattern.strip_suffix('/') {
        Some(p) => (p, true),
        None => (pattern, false),
    };
    let mut have = path.split('/');
    for want in pattern.split('/') {
        if !have.next().is_some_and(|h| segment(want, h)) {
            return false;
        }
    }
    under != have.next().is_none()
}

/// `*` matches any run within one path segment.
fn segment(want: &str, have: &str) -> bool {
    let Some((head, tail)) = want.split_once('*') else {
        return want == have;
    };
    let Some(rest) = have.strip_prefix(head) else {
        return false;
    };
    (0..=rest.len()).any(|i| rest.is_char_boundary(i) && segment(tail, &rest[i..]))
}
