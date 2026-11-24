//! Failed and slow tests, from the JUnit file nextest writes.

use stickler_findings::{Finding, Level, Report, Rule, Snippet};

/// The same limit as the `ci` profile in .config/nextest.toml.
const SLOW_SECS: f64 = 30.0;
/// Output lines kept from the end of a failure.
const TAIL: usize = 40;

/// The last `TAIL` lines of `text`.
fn tail(text: &str) -> String {
    let lines: Vec<&str> = text.trim_end().lines().collect();
    lines[lines.len().saturating_sub(TAIL)..].join("\n")
}

fn finding(rule: &str, level: Level, message: String, output: Option<String>) -> Finding {
    Finding {
        rule: rule.into(),
        level,
        path: String::new(),
        line: 0,
        message,
        snippets: output
            .map(|text| Snippet {
                path: String::new(),
                line: 0,
                text,
            })
            .into_iter()
            .collect(),
        fix: None,
    }
}

/// A report from the JUnit XML in `xml`.
pub fn report(xml: &str) -> Result<Report, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    let mut r = Report::new("tests", "Tests");
    let rule = |name: &str, fix: &str| Rule {
        name: name.into(),
        fix: fix.into(),
    };
    r.rules.insert(
        "failed".into(),
        rule(
            "Failed test",
            "Run it with `cargo nextest run`, and fix the cause.",
        ),
    );
    r.rules.insert(
        "slow".into(),
        rule(
            "Slow test",
            "Make the test smaller, or split it into parts.",
        ),
    );
    for case in doc.descendants().filter(|n| n.has_tag_name("testcase")) {
        let name = format!(
            "{} {}",
            case.attribute("classname").unwrap_or_default(),
            case.attribute("name").unwrap_or_default()
        );
        let child = |tag: &str| case.children().find(|n| n.has_tag_name(tag));
        let text = |tag: &str| child(tag).and_then(|n| n.text()).unwrap_or_default();
        let failed = child("failure").or_else(|| child("error"));
        if let Some(f) = failed {
            let why = f.attribute("type").unwrap_or("failed");
            let output = match text("system-err").trim().is_empty() {
                true => text("system-out"),
                false => text("system-err"),
            };
            let message = format!("`{name}`: {why}");
            r.findings
                .push(finding("failed", Level::Fail, message, Some(tail(output))));
            continue;
        }
        let secs: f64 = case
            .attribute("time")
            .and_then(|t| t.parse().ok())
            .unwrap_or(0.0);
        if secs > SLOW_SECS {
            let message = format!("`{name}` took {secs:.1} s.");
            r.findings.push(finding("slow", Level::Note, message, None));
        }
    }
    Ok(r)
}
