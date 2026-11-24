use crate::cache::{Cache, KEEP_SECS, key, now};
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/target/scratch"));
    let path = dir.join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    path
}

#[test]
fn cache_keeps_week() {
    let dir = scratch("week");
    let mut c = Cache::default();
    c.insert(key("a"), vec![1.0, 2.0]);
    c.insert(key("b"), vec![3.0, 4.0]);
    // Eight days on, only the entry used then is left.
    let later = now() + KEEP_SECS + 86_400;
    c.save_at(&dir, &[key("a")], later).unwrap();
    let back = Cache::load(&dir).unwrap();
    assert_eq!(back.get(key("a")), Some(&vec![1.0, 2.0]));
    assert!(back.get(key("b")).is_none() && key("a") != key("b"));
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(Cache::load(&dir).unwrap().get(key("a")).is_none());
}

/// Two runs from one cache keep both new entries.
#[test]
fn saves_merge() {
    let dir = scratch("merge");
    let (mut first, mut second) = (Cache::load(&dir).unwrap(), Cache::load(&dir).unwrap());
    first.insert(key("a"), vec![1.0, 2.0]);
    second.insert(key("b"), vec![3.0, 4.0]);
    first.save(&dir, &[key("a")]).unwrap();
    second.save(&dir, &[key("b")]).unwrap();
    let back = Cache::load(&dir).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(back.get(key("a")).is_some() && back.get(key("b")).is_some());
}
