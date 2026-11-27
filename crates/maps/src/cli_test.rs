use crate::cli::Command;

const TREE: &str = r#"{
  "flags": [{ "flag": "--model", "takes": true }],
  "commands": {
    "serve": { "flags": [{ "flag": "--attic", "takes": false }] },
    "ask": { "words": true }
  }
}"#;

#[test]
fn walks_args() {
    let tree: Command = serde_json::from_str(TREE).unwrap();
    let check = |args: &[&str]| {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        tree.check(&args).err().unwrap_or_default()
    };
    assert_eq!(check(&["--model", "<dir>", "serve", "--attic"]), "");
    assert_eq!(check(&["ask", "serve", "<prompt>"]), "");
    let cases = [
        (&["serve", "--port"][..], "no flag `--port`"),
        (&["--model"], "needs a value"),
        (&["serve", "--attic=1"], "takes no value"),
        (&["serve", "x"], "takes no word"),
    ];
    for (args, want) in cases {
        assert!(check(args).contains(want), "{args:?}: {}", check(args));
    }
}
