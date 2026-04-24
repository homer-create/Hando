use anyhow::Result;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum DisposalKind {
    Trashed,
    RenamedFallback { backup_path: PathBuf },
}

#[derive(Debug, Clone)]
pub struct Disposal {
    pub original_path: PathBuf,
    pub kind: DisposalKind,
}

/// Strip the `\\?\` extended-path prefix that Windows `canonicalize()` adds.
/// `trash::os_limited::list()` returns plain paths; without stripping,
/// the comparison fails and Undo can't find the item in the Recycle Bin.
fn strip_extended_prefix(p: PathBuf) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let s = p.to_string_lossy();
        if let Some(stripped) = s.strip_prefix("\\\\?\\") {
            return PathBuf::from(stripped);
        }
    }
    p
}

pub fn dispose_original(path: &Path) -> Result<Disposal> {
    let abs = strip_extended_prefix(path.canonicalize()?);
    match trash::delete(&abs) {
        Ok(()) => Ok(Disposal { original_path: abs, kind: DisposalKind::Trashed }),
        Err(primary) => {
            let backup = fallback_backup_path(&abs);
            std::fs::rename(&abs, &backup).map_err(|secondary| {
                anyhow::anyhow!("trash failed ({primary}); rename fallback also failed: {secondary}")
            })?;
            Ok(Disposal {
                original_path: abs,
                kind: DisposalKind::RenamedFallback { backup_path: backup },
            })
        }
    }
}

fn fallback_backup_path(src: &Path) -> PathBuf {
    let stem = src.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = src.extension().map(|s| format!(".{}", s.to_string_lossy())).unwrap_or_default();
    let parent = src.parent().unwrap_or_else(|| Path::new("."));
    let mut candidate = parent.join(format!("{stem}.original{ext}"));
    let mut n = 2;
    while candidate.exists() {
        candidate = parent.join(format!("{stem}.original-{n}{ext}"));
        n += 1;
    }
    candidate
}

pub fn restore_all(disposals: &[Disposal]) -> Result<usize> {
    let mut restored = 0;
    let trashed_targets: Vec<&PathBuf> = disposals
        .iter()
        .filter_map(|d| matches!(d.kind, DisposalKind::Trashed).then_some(&d.original_path))
        .collect();
    if !trashed_targets.is_empty() {
        match trash::os_limited::list() {
            Ok(items) => {
                for want in &trashed_targets {
                    let want_norm = strip_extended_prefix((*want).clone());
                    if let Some(item) = items.iter().find(|i| {
                        let item_path = strip_extended_prefix(PathBuf::from(&i.original_parent).join(&i.name));
                        item_path == want_norm
                    }) {
                        if trash::os_limited::restore_all([item.clone()]).is_ok() {
                            restored += 1;
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("trash::os_limited::list() failed: {e}");
            }
        }
    }
    for d in disposals {
        if let DisposalKind::RenamedFallback { backup_path } = &d.kind {
            if std::fs::rename(backup_path, &d.original_path).is_ok() {
                restored += 1;
            }
        }
    }
    Ok(restored)
}
