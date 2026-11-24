use crate::tests::report;
use stickler_findings::Level;

const XML: &str = r#"<testsuites><testsuite name="linear">
<testcase name="quick" classname="linear" time="0.1"><system-out>ok</system-out></testcase>
<testcase name="long" classname="linear" time="45.0"/>
<testcase name="broken" classname="linear" time="0.2">
<failure type="test failure with exit code 101"/>
<system-out>running 1 test</system-out>
<system-err>thread 'broken' panicked at a.rs:3:5:
want 1, got 2</system-err>
</testcase>
</testsuite></testsuites>"#;

#[test]
fn reads_junit() {
    let r = report(XML).unwrap();
    let found: Vec<(&str, Level)> = r
        .findings
        .iter()
        .map(|f| (f.rule.as_str(), f.level))
        .collect();
    assert!(found == [("slow", Level::Note), ("failed", Level::Fail)]);
    let failed = &r.findings[1];
    assert!(
        failed
            .message
            .contains("`linear broken`: test failure with exit code 101")
    );
    assert!(failed.snippets[0].text.ends_with("want 1, got 2"));
}
