use crate::cache::{Cache, KEEP_SECS, SHARDS, key, now};
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

/// An f32 shard loads, then saves at half size.
#[test]
fn f32_shard_loads() {
    let dir = scratch("f32");
    std::fs::create_dir_all(&dir).unwrap();
    let k = key("a");
    let mut bytes = b"simck002".to_vec();
    bytes.extend(2u32.to_le_bytes());
    bytes.extend(k.to_le_bytes());
    bytes.extend(now().to_le_bytes());
    bytes.extend([0.5f32, 0.25].iter().flat_map(|f| f.to_le_bytes()));
    let shard = dir.join(format!("{:02x}.bin", k % SHARDS));
    std::fs::write(&shard, &bytes).unwrap();
    let mut c = Cache::load(&dir).unwrap();
    assert_eq!(c.get(k), Some(&vec![0.5, 0.25]));
    c.save(&dir, &[k]).unwrap();
    let saved = std::fs::metadata(&shard).unwrap().len();
    assert_eq!(Cache::load(&dir).unwrap().get(k), Some(&vec![0.5, 0.25]));
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(saved as usize, bytes.len() - 4);
}
