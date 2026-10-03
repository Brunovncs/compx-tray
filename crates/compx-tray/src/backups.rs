//! Saved copies of a profile's settings, as JSON files in `%LOCALAPPDATA%\compx-tray\backups`.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use compx_proto::ipc::Backup;
use compx_proto::settings::MouseSettings;

#[derive(Serialize, Deserialize)]
struct File {
    model: String,
    profile: u8,
    created: u64,
    settings: MouseSettings,
}

fn dir() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("compx-tray").join("backups"))
}

/// Only plain file names from our own folder: the name comes from the panel.
fn path(name: &str) -> Option<PathBuf> {
    let safe = name.ends_with(".json") && !name.contains(['/', '\\', ':']) && !name.starts_with('.');
    safe.then(|| dir().map(|d| d.join(name)))?
}

pub fn save(model: &str, profile: u8, settings: MouseSettings) -> std::io::Result<()> {
    let dir = dir().ok_or(std::io::ErrorKind::NotFound)?;
    std::fs::create_dir_all(&dir)?;
    let created = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let file = File { model: model.to_owned(), profile, created, settings };
    let json = serde_json::to_vec_pretty(&file).map_err(std::io::Error::other)?;
    std::fs::write(dir.join(format!("profile{}-{created}.json", profile + 1)), json)
}

pub fn load(name: &str) -> Option<MouseSettings> {
    let bytes = std::fs::read(path(name)?).ok()?;
    serde_json::from_slice::<File>(&bytes).ok().map(|f| f.settings)
}

pub fn delete(name: &str) {
    if let Some(p) = path(name) {
        let _ = std::fs::remove_file(p);
    }
}

/// Newest first.
pub fn list() -> Vec<Backup> {
    let Some(entries) = dir().and_then(|d| std::fs::read_dir(d).ok()) else { return Vec::new() };
    let mut list: Vec<Backup> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let file: File = serde_json::from_slice(&std::fs::read(e.path()).ok()?).ok()?;
            Some(Backup { name, model: file.model, profile: file.profile, created: file.created })
        })
        .collect();
    list.sort_by_key(|b| std::cmp::Reverse(b.created));
    list
}
