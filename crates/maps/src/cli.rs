//! A binary's `cli.json` command tree, and `run` step words.

use std::collections::BTreeMap;

use serde::Deserialize;

/// The file beside a package's `Cargo.toml` that its binary prints.
pub(crate) const FILE: &str = "cli.json";

#[derive(Deserialize)]
pub struct Flag {
    pub flag: String,
    /// Whether the next word is its value.
    pub takes: bool,
}

#[derive(Deserialize, Default)]
pub struct Command {
    #[serde(default)]
    pub flags: Vec<Flag>,
    /// Whether it takes words that are not subcommands.
    #[serde(default)]
    pub words: bool,
    #[serde(default)]
    pub commands: BTreeMap<String, Command>,
}

fn placeholder(arg: &str) -> bool {
    arg.len() > 2 && arg.starts_with('<') && arg.ends_with('>')
}

impl Command {
    /// The first flag named `name` here or in a parent.
    fn flag<'a>(path: &[&'a Command], name: &str) -> Option<&'a Flag> {
        for c in path.iter().rev() {
            if let Some(f) = c.flags.iter().find(|f| f.flag == name) {
                return Some(f);
            }
        }
        None
    }

    /// Whether `args` parse: subcommands, then known flags and words.
    pub fn check(&self, args: &[String]) -> Result<(), String> {
        let mut path = vec![self];
        let mut shown: Vec<&str> = Vec::new();
        let mut value: Option<&str> = None;
        let mut words = false;
        for arg in args {
            let at = || match shown.is_empty() {
                true => "the binary".to_string(),
                false => format!("`{}`", shown.join(" ")),
            };
            if value.take().is_some() {
                continue;
            }
            let here = path[path.len() - 1];
            if arg.len() > 1 && arg.starts_with('-') {
                let (name, inline) = match arg.split_once('=') {
                    Some((name, _)) => (name, true),
                    None => (arg.as_str(), false),
                };
                let flag = Command::flag(&path, name)
                    .ok_or_else(|| format!("{} has no flag `{name}`", at()))?;
                match (flag.takes, inline) {
                    (true, false) => value = Some(name),
                    (false, true) => return Err(format!("`{name}` takes no value")),
                    _ => {}
                }
                continue;
            }
            if let (false, false, Some(sub)) = (placeholder(arg), words, here.commands.get(arg)) {
                path.push(sub);
                shown.push(arg);
                continue;
            }
            if !here.words {
                return Err(format!("{} takes no word `{arg}`", at()));
            }
            words = true;
        }
        match value {
            Some(name) => Err(format!("`{name}` needs a value")),
            None => Ok(()),
        }
    }
}
