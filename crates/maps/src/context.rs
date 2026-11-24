//! A task with the code each step cites, for prompts.

use std::fmt::Write;

use crate::Atlas;
use crate::code::{Item, Kind};

/// Lines printed per cited item before the rest is cut.
const SPAN: usize = 60;
/// Lines kept around each match in a whole file.
const AROUND: usize = 2;

/// The word a reference names, to find in other files.
fn needle(value: &str) -> Option<&str> {
    if value.contains("::") {
        return value.trim_end_matches('!').rsplit("::").next();
    }
    let mut parts = value.rsplit('/');
    let file = parts.next()?;
    if let Some((_, tail)) = file.rsplit_once('*') {
        return tail.starts_with('.').then_some(tail);
    }
    let stem = file.split('.').next()?;
    match stem {
        "mod" => parts.next(),
        "lib" | "main" => None,
        _ => Some(stem),
    }
}

/// `word` in `line`, not as part of a longer name.
fn names(line: &str, word: &str) -> bool {
    if word.starts_with('.') {
        return line.contains(word);
    }
    let edge = |c: Option<char>| !c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    for (i, _) in line.match_indices(word) {
        if edge(line[..i].chars().next_back()) && edge(line[i + word.len()..].chars().next()) {
            return true;
        }
    }
    false
}

/// Line ranges to print, first and last inclusive.
fn ranges(text: &str, it: &Item, words: &[&str]) -> Vec<(usize, usize)> {
    let n = text.lines().count();
    if !matches!(it.kind, Kind::File | Kind::Mod) {
        return vec![(it.start.max(1), it.end.min(n))];
    }
    let mut out: Vec<(usize, usize)> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if !words.iter().any(|w| names(line, w)) {
            continue;
        }
        let (a, b) = (
            (i + 1).saturating_sub(AROUND).max(1),
            (i + 1 + AROUND).min(n),
        );
        match out.last_mut() {
            Some(last) if a <= last.1 + 1 => last.1 = b,
            _ => out.push((a, b)),
        }
    }
    out
}

fn excerpt(out: &mut String, text: &str, file: &str, spans: &[(usize, usize)]) {
    let lines: Vec<&str> = text.lines().collect();
    if spans.is_empty() {
        let _ = writeln!(out, "    {file}");
    }
    let mut left = SPAN;
    for &(a, b) in spans {
        let _ = writeln!(out, "    {file}:{a}-{b}\n    ```rust");
        for at in a..=b {
            if left == 0 {
                let _ = writeln!(out, "    (cut at {SPAN} lines)");
                break;
            }
            let _ = writeln!(out, "    {at:>5} {}", lines[at - 1]);
            left -= 1;
        }
        let _ = writeln!(out, "    ```");
    }
}

impl Atlas {
    /// Task `id`, each reference with the code it resolves to.
    pub fn context(&self, id: &str) -> Result<String, String> {
        let map = self.task(id)?;
        let mut out = format!(
            "task {} {} digest {:016x}\n",
            map.id,
            map.path,
            self.digest_of(map)
        );
        for (n, step) in map.task.steps.iter().enumerate() {
            let s = step.get_ref();
            let mode = s.mode.map(|m| format!(" mode={m:?}")).unwrap_or_default();
            let _ = writeln!(
                out,
                "\nstep {} {}{}",
                n + 1,
                s.verb.name(),
                mode.to_lowercase()
            );
            let refs = self.refs(s);
            for (field, value, got) in &refs {
                let _ = writeln!(out, "  {field} {value}");
                let items = match got {
                    Ok(items) => items,
                    Err(why) => {
                        let _ = writeln!(out, "    unresolved: {why}");
                        continue;
                    }
                };
                let words: Vec<&str> = refs
                    .iter()
                    .filter(|r| r.1 != *value)
                    .filter_map(|r| needle(&r.1))
                    .collect();
                for it in items {
                    match self.repo.text(&it.file) {
                        Some(text) => excerpt(&mut out, text, &it.file, &ranges(text, it, &words)),
                        None => {
                            let _ = writeln!(out, "    {}", it.file);
                        }
                    }
                }
            }
        }
        for e in &map.task.each {
            let e = e.get_ref();
            let _ = writeln!(out, "\neach {} has {}", e.of, e.has);
            let reg = self.inv.find(&self.repo.resolve(&e.of), &e.of);
            for m in reg.map_or(&[][..], |r| &self.inv.registries[r].members[..]) {
                let _ = writeln!(out, "  {} {}:{}", m.key, m.file, m.line);
            }
        }
        Ok(out)
    }
}
