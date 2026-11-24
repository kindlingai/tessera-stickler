//! Dependencies on workspace crates that carry only a few names.

use std::collections::BTreeSet;

use super::File;
use crate::Violation;
use crate::audit::workspace::{Kind, Pkg};

/// Names `p` uses from `dep`, or `None` for a glob.
fn named<'a>(files: &'a [File], p: &Pkg, dep: &str, kind: Kind) -> Option<BTreeSet<&'a str>> {
    let mut items = BTreeSet::new();
    for f in files.iter().filter(|f| f.pkg.name == p.name) {
        let list = if kind == Kind::Normal {
            &f.code
        } else {
            &f.test
        };
        for n in list.iter().filter(|n| n.segs[0] == dep) {
            match n.segs[1].as_str() {
                "*" | "_" => return None,
                "self" => {}
                name => {
                    items.insert(name);
                }
            }
        }
    }
    Some(items)
}

/// Limits of the thin-edge rule, from lint.toml.
#[derive(serde::Deserialize)]
pub struct Thin {
    pub dirs: Vec<String>,
    pub min_names: usize,
    pub min_dev_names: usize,
    pub min_surface: usize,
}

/// Top-level names of `dep` that other crates use.
fn surface(files: &[File], dep: &str) -> usize {
    let mut all = BTreeSet::new();
    for f in files.iter().filter(|f| f.pkg.name.replace('-', "_") != dep) {
        let paths = f.code.iter().chain(&f.test);
        all.extend(
            paths
                .filter(|n| n.segs[0] == dep)
                .map(|n| n.segs[1].as_str()),
        );
    }
    all.len()
}

/// Dependencies on the lowest layer, `base`, pass.
pub fn check(pkgs: &[Pkg], files: &[File], base: &[String], thin: &Thin, out: &mut Vec<Violation>) {
    let checked = pkgs
        .iter()
        .filter(|p| thin.dirs.iter().any(|d| p.dir.starts_with(d)));
    for p in checked {
        for d in p.deps.iter().filter(|d| d.kind != Kind::Build) {
            let low = pkgs.iter().find(|q| q.name == d.name);
            if low.is_some_and(|q| base.iter().any(|b| q.dir.starts_with(b))) {
                continue;
            }
            let dep = d.name.replace('-', "_");
            let Some(items) = named(files, p, &dep, d.kind) else {
                continue;
            };
            let min = if d.kind == Kind::Normal {
                thin.min_names
            } else {
                thin.min_dev_names
            };
            let all = surface(files, &dep);
            if items.len() >= min || all < thin.min_surface {
                continue;
            }
            let list: Vec<String> = items.iter().map(|i| format!("`{i}`")).collect();
            let message = format!(
                "`{}` {} `{}` for {} of the {} names other crates use ({}): move them or drop the dependency",
                p.name,
                d.kind.verb(),
                d.name,
                items.len(),
                all,
                list.join(", ")
            );
            out.push(Violation {
                path: p.manifest_path(),
                line: p.dep_line(&d.name, d.kind),
                rule: "thin-edge",
                message,
            });
        }
    }
}
