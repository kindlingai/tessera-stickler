//! Task maps: ordered verb steps over checked code references.

mod calls;
mod code;
mod context;
mod map;
mod query;
mod registry;
mod verbs;

#[cfg(test)]
mod verbs_test;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::LazyLock;

pub use code::listed;
use code::{Item, Repo};
use map::{Map, Step};
use registry::Inventory;
use verbs::Book;

/// The lint.toml limits maps read.
#[derive(serde::Deserialize)]
struct Limits {
    max_name_segments: usize,
    max_map_id_chars: usize,
    max_map_steps: usize,
    maps_dir: String,
}

/// The repository this program runs in.
fn repo_root() -> Result<PathBuf, String> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err("git rev-parse --show-toplevel failed".into());
    }
    Ok(PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()))
}

static LIMITS: LazyLock<Limits> = LazyLock::new(|| {
    let path = repo_root().map_or_else(|e| panic!("{e}"), |r| r.join("lint.toml"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    toml::from_str(&text).unwrap_or_else(|e| panic!("lint.toml: {e}"))
});

#[derive(Clone)]
pub struct Fault {
    pub path: String,
    pub line: usize,
    pub message: String,
}

/// The maps, the workspace they cite and its registries.
pub(crate) struct Atlas {
    repo: Repo,
    inv: Inventory,
    maps: Vec<Map>,
    texts: BTreeMap<String, String>,
    faults: Vec<Fault>,
}

/// A reference as written, and what it resolved to.
type Ref = (&'static str, String, Result<Vec<Item>, String>);

impl Atlas {
    pub fn open(root: &Path) -> Result<Atlas, String> {
        let repo = Repo::open(root)?;
        let (mut maps, mut texts, mut faults) = (Vec::new(), BTreeMap::new(), Vec::new());
        let dir = LIMITS.maps_dir.as_str();
        for path in repo.files.iter().filter(|f| f.starts_with(dir)) {
            let line = 1;
            if !path.ends_with(".toml") || path[dir.len()..].contains('/') {
                let message = format!("only {dir}<id>.toml files belong here");
                faults.push(Fault {
                    path: path.clone(),
                    line,
                    message,
                });
                continue;
            }
            let text =
                std::fs::read_to_string(root.join(path)).map_err(|e| format!("{path}: {e}"))?;
            maps.extend(map::read(path, &text, &mut faults));
            texts.insert(path.clone(), text);
        }
        let inv = registry::derive(&repo);
        Ok(Atlas {
            repo,
            inv,
            maps,
            texts,
            faults,
        })
    }

    fn book(&self) -> Book<'_> {
        Book {
            repo: &self.repo,
            inv: &self.inv,
        }
    }

    fn line(&self, map: &Map, offset: usize) -> usize {
        map::line_of(&self.texts[&map.path], offset)
    }

    fn refs(&self, step: &Step) -> Vec<Ref> {
        let book = self.book();
        let mut out = Vec::new();
        for (field, value, class) in step.refs() {
            out.push((field, value.to_string(), book.resolve(value, class)));
        }
        out
    }

    /// Every fault: format, resolution, relation and invariant.
    pub fn check(&self) -> Vec<Fault> {
        let mut out = self.faults.clone();
        let book = self.book();
        let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
        for map in &self.maps {
            if let Some(first) = seen.insert(&map.id, &map.path) {
                out.push(map.fault(map.line, format!("is also in {first}")));
            }
            for (n, step) in map.task.steps.iter().enumerate() {
                let line = self.line(map, step.span().start);
                let head = format!("step {} ({})", n + 1, step.get_ref().verb.name());
                let mut like = Vec::new();
                let mut resolved = true;
                for (field, _, got) in self.refs(step.get_ref()) {
                    match got {
                        Ok(items) if field == "like" => like = items,
                        Ok(_) => {}
                        Err(why) => {
                            resolved = false;
                            out.push(map.fault(line, format!("{head} {field}: {why}")));
                        }
                    }
                }
                if let (true, Err(why)) = (resolved, book.holds(step.get_ref(), &like)) {
                    out.push(map.fault(line, format!("{head} does not hold: {why}")));
                }
            }
            for e in &map.task.each {
                let line = self.line(map, e.span().start);
                if let Err(why) = book.each(e.get_ref()) {
                    out.push(map.fault(
                        line,
                        format!("each {} does not hold: {why}", e.get_ref().of),
                    ));
                }
            }
        }
        out
    }
}

/// Map faults in the repository at `root`.
pub fn check(root: &Path) -> Result<Vec<Fault>, String> {
    Ok(Atlas::open(root)?.check())
}

const USAGE: &str = "usage: stickler maps [check | show <task> | context <task> | cites <path> | registries | digest]";

pub fn main(args: &[&str]) -> ExitCode {
    stickler_findings::main_of("stickler maps", run, args)
}

fn run(args: &[&str]) -> Result<bool, String> {
    let root = repo_root()?;
    let atlas = Atlas::open(&root)?;
    let faults = atlas.check();
    let check = matches!(args, [] | ["check"]);
    for f in &faults {
        let line = format!("{}:{}: map: {}", f.path, f.line, f.message);
        match check {
            true => println!("{line}"),
            false => eprintln!("{line}"),
        }
    }
    let out = match args {
        [] | ["check"] => return Ok(faults.is_empty()),
        ["show", id] => atlas.show(id)?,
        ["context", id] => atlas.context(id)?,
        ["cites", path] => atlas.cites(path),
        ["registries"] => atlas.inventory(),
        ["digest"] => atlas.digest(),
        _ => return Err(USAGE.into()),
    };
    print!("{out}");
    Ok(true)
}
