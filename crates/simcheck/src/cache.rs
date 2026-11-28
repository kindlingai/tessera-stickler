//! Embeddings by window text, in shard files merged on save.

use crate::fetch::REVISION;
use anyhow::{Result, ensure};
use half::f16;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Changes with the layout. Entries hold last use, f16 values.
const MAGIC: &[u8; 8] = b"simck003";
/// The f32 layout. It loads, then saves as `MAGIC`.
const F32_MAGIC: &[u8; 8] = b"simck002";
/// Saving drops entries unused for this long.
pub(crate) const KEEP_SECS: u64 = 7 * 24 * 60 * 60;
pub(crate) const SHARDS: u64 = 16;
/// Keep in step with eg2cpu's `rev` in Cargo.toml.
const EG2CPU: &str = "a80156abbf8ec2e5a9b85ed0e5ff98454c1aaa09";

/// FNV-1a over the model revisions and the window text.
pub fn key(text: &str) -> u64 {
    let parts = [EG2CPU.as_bytes(), REVISION.as_bytes(), text.as_bytes()];
    parts
        .iter()
        .flat_map(|p| p.iter())
        .fold(0xcbf2_9ce4_8422_2325, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
        })
}

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn shard_path(dir: &Path, k: u64) -> PathBuf {
    dir.join(format!("{:02x}.bin", k % SHARDS))
}

/// Each key's last use in Unix seconds, and its embedding.
type Entries = HashMap<u64, (u64, Vec<f32>)>;

#[derive(Default)]
pub(crate) struct Cache {
    dim: usize,
    map: Entries,
}

/// Values of `width` bytes, f16 or f32.
fn floats(bytes: &[u8], width: usize) -> Vec<f32> {
    match width {
        2 => bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|f| f16::from_le_bytes(*f).to_f32())
            .collect(),
        _ => bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|f| f32::from_le_bytes(*f))
            .collect(),
    }
}

/// One file's entries, or none if missing or unknown.
fn read_file(path: &Path) -> Result<(usize, Entries)> {
    let Ok(bytes) = std::fs::read(path) else {
        return Ok((0, Entries::new()));
    };
    let width = match bytes.get(..8) {
        Some(m) if m == MAGIC => 2,
        Some(m) if m == F32_MAGIC => 4,
        _ => return Ok((0, Entries::new())),
    };
    if bytes.len() < 12 {
        return Ok((0, Entries::new()));
    }
    let dim = u32::from_le_bytes(bytes[8..12].try_into()?) as usize;
    let entry = 16 + dim * width;
    let body = &bytes[12..];
    ensure!(
        dim > 0 && body.len() % entry == 0,
        "{}: truncated",
        path.display()
    );
    let mut map = Entries::new();
    for e in body.chunks_exact(entry) {
        let k = u64::from_le_bytes(e[..8].try_into()?);
        let used = u64::from_le_bytes(e[8..16].try_into()?);
        map.insert(k, (used, floats(&e[16..], width)));
    }
    Ok((dim, map))
}

/// Writes `entries` to a temporary file, renamed to `path`.
fn write_file(path: &Path, dim: usize, entries: &Entries) -> Result<()> {
    let mut keys: Vec<u64> = entries.keys().copied().collect();
    keys.sort_unstable();
    let mut out = MAGIC.to_vec();
    out.extend((dim as u32).to_le_bytes());
    for k in keys {
        let (used, v) = &entries[&k];
        out.extend(k.to_le_bytes());
        out.extend(used.to_le_bytes());
        out.extend(v.iter().flat_map(|f| f16::from_f32(*f).to_le_bytes()));
    }
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&tmp, out)?;
    Ok(std::fs::rename(&tmp, path)?)
}

impl Cache {
    /// The cache in `dir`, or an empty one.
    pub fn open(dir: Option<&Path>) -> Result<Cache> {
        dir.map_or_else(|| Ok(Cache::default()), Cache::load)
    }

    /// Every shard in `dir`, or the older single file `dir.bin`.
    pub fn load(dir: &Path) -> Result<Cache> {
        let mut c = Cache::default();
        let files: Vec<PathBuf> = match dir.is_dir() {
            true => (0..SHARDS).map(|i| shard_path(dir, i)).collect(),
            false => vec![dir.with_extension("bin")],
        };
        for f in files {
            let (dim, map) = read_file(&f)?;
            c.dim = c.dim.max(dim);
            c.map.extend(map);
        }
        Ok(c)
    }

    pub fn get(&self, k: u64) -> Option<&Vec<f32>> {
        self.map.get(&k).map(|(_, v)| v)
    }

    pub fn insert(&mut self, k: u64, v: Vec<f32>) {
        self.dim = v.len();
        self.map.insert(k, (now(), v));
    }

    /// Marks `used` at `at`, then merges each shard on disk.
    pub fn save_at(&mut self, dir: &Path, used: &[u64], at: u64) -> Result<()> {
        for k in used {
            if let Some(e) = self.map.get_mut(k) {
                e.0 = at;
            }
        }
        std::fs::create_dir_all(dir)?;
        for shard in 0..SHARDS {
            let path = shard_path(dir, shard);
            // Entries another run saved since this one loaded stay.
            let (_, mut merged) = read_file(&path)?;
            for (k, (when, v)) in self.map.iter().filter(|(k, _)| *k % SHARDS == shard) {
                let e = merged.entry(*k).or_insert((*when, v.clone()));
                e.0 = e.0.max(*when);
            }
            merged.retain(|_, (when, _)| at.saturating_sub(*when) <= KEEP_SECS);
            write_file(&path, self.dim, &merged)?;
        }
        Ok(())
    }

    pub fn save(&mut self, dir: &Path, used: &[u64]) -> Result<()> {
        self.save_at(dir, used, now())
    }
}
