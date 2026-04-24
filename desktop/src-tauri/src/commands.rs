// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::batch::BatchState;
use crate::sidecar::{EncodeCommand, EncodeOpts, Sidecar, SidecarEvent};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

#[derive(Default, Clone)]
pub struct SidecarState(pub Arc<Mutex<Option<Sidecar>>>);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressArgs {
    pub batch_id: String,
    pub files: Vec<CompressFile>,
    pub opts: EncodeOpts,
    pub move_originals_to_trash: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressFile {
    pub id: String,
    pub path: String,
    pub ext: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileDonePayload {
    pub id: String,
    pub tmp: String,
    pub src_bytes: u64,
    pub out_bytes: u64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileErrorPayload {
    pub id: String,
    pub msg: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoReport { pub restored: usize, pub attempted: usize }

/// Resolve a resource path, checking in order:
/// 1. Tauri's `resource_dir()` (installed / MSI / NSIS)
/// 2. The directory containing the running executable (portable mode)
/// 3. The dev fallback relative to `CARGO_MANIFEST_DIR`
fn resolve_resource(app: &AppHandle, rel: &str, dev_rel: &str) -> PathBuf {
    if let Ok(dir) = app.path().resource_dir() {
        let candidate = dir.join(rel);
        if candidate.exists() { return candidate; }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let candidate = exe_dir.join(rel);
            if candidate.exists() { return candidate; }
        }
    }
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    here.join(dev_rel).canonicalize().unwrap_or(here)
}

fn sidecar_script_path(app: &AppHandle) -> PathBuf {
    resolve_resource(app, "src/sidecar.js", "../../src/sidecar.js")
}

fn node_binary(app: &AppHandle) -> PathBuf {
    #[cfg(target_os = "windows")]
    let name = "node.exe";
    #[cfg(not(target_os = "windows"))]
    let name = "node";

    if let Ok(dir) = app.path().resource_dir() {
        let bundled = dir.join(name);
        if bundled.exists() { return bundled; }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let candidate = exe_dir.join(name);
            if candidate.exists() { return candidate; }
        }
    }
    PathBuf::from("node")
}

async fn ensure_sidecar(app: &AppHandle, state: &SidecarState) -> Result<(), String> {
    let mut guard = state.0.lock().await;
    if guard.is_some() { return Ok(()); }
    let script = sidecar_script_path(app);
    let node = node_binary(app);
    let sc = Sidecar::spawn(app.clone(), node, script)
        .await
        .map_err(|e| e.to_string())?;
    *guard = Some(sc);
    Ok(())
}

pub async fn on_sidecar_crashed(sc_state: &SidecarState) {
    *sc_state.0.lock().await = None;
}

async fn place_file(tmp: &str, dest: &str) -> Result<(), String> {
    if let Err(_) = tokio::fs::rename(tmp, dest).await {
        match tokio::fs::copy(tmp, dest).await {
            Ok(_) => { let _ = tokio::fs::remove_file(tmp).await; }
            Err(e) => {
                let _ = tokio::fs::remove_file(tmp).await;
                return Err(format!("could not place file: {e}"));
            }
        }
    }
    Ok(())
}

async fn apply_done(
    app: &AppHandle,
    batch_id: &str,
    id: &str,
    src_path: &str,
    tmp: &str,
    src_bytes: u64,
    out_bytes: u64,
    move_to_trash: bool,
    companions: &[crate::sidecar::Companion],
    batches: &State<'_, BatchState>,
) -> Result<(), String> {
    if move_to_trash {
        let disposal = crate::trash::dispose_original(Path::new(src_path)).map_err(|e| e.to_string())?;
        let kind_note = match &disposal.kind {
            crate::trash::DisposalKind::Trashed => None,
            crate::trash::DisposalKind::RenamedFallback { backup_path } =>
                Some(format!("Trash unavailable; original backed up to {}", backup_path.display())),
        };
        batches.record_disposal(batch_id, disposal);
        if let Some(note) = kind_note {
            let _ = app.emit("trash-fallback", serde_json::json!({ "id": id, "note": note }));
        }
    } else {
        tokio::fs::remove_file(src_path).await.map_err(|e| e.to_string())?;
    }

    place_file(tmp, src_path).await?;

    // Place WebP/AVIF companions alongside the source file and record their paths for Undo
    let mut companion_dests: Vec<PathBuf> = vec![];
    for c in companions {
        let src_p = Path::new(src_path);
        let stem = src_p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let parent = src_p.parent().unwrap_or_else(|| Path::new("."));
        let dest = parent.join(format!("{stem}{}", c.ext));
        match place_file(&c.tmp, &dest.to_string_lossy()).await {
            Ok(()) => companion_dests.push(dest),
            Err(e) => { let _ = app.emit("companion-error", serde_json::json!({ "id": id, "ext": c.ext, "msg": e })); }
        }
    }
    if move_to_trash {
        // Record companion paths so Undo can delete them
        batches.record_companion_paths(batch_id, companion_dests);
    }

    let _ = app.emit("file-done", FileDonePayload {
        id: id.to_string(),
        tmp: tmp.to_string(),
        src_bytes,
        out_bytes,
    });
    Ok(())
}

#[tauri::command]
pub async fn compress(
    app: AppHandle,
    sc_state: State<'_, SidecarState>,
    batches: State<'_, BatchState>,
    args: CompressArgs,
) -> Result<(), String> {
    ensure_sidecar(&app, &sc_state).await?;
    batches.start(args.batch_id.clone());

    let mut src_by_id: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for f in &args.files { src_by_id.insert(f.id.clone(), f.path.clone()); }
    let mut pending = args.files.len();

    {
        let guard = sc_state.0.lock().await;
        let sc = guard.as_ref().ok_or("sidecar missing")?;
        for f in &args.files {
            sc.send(&EncodeCommand {
                cmd: "encode",
                id: f.id.clone(),
                src: f.path.clone(),
                ext: f.ext.clone(),
                opts: args.opts.clone(),
            }).await.map_err(|e| e.to_string())?;
        }
    }

    let batch_id = args.batch_id.clone();
    let move_to_trash = args.move_originals_to_trash;
    let app_c = app.clone();
    let sc_arc = sc_state.inner().0.clone();
    tokio::spawn(async move {
        let batches = app_c.state::<BatchState>();
        while pending > 0 {
            let evt = {
                let mut guard = sc_arc.lock().await;
                match guard.as_mut() {
                    Some(sc) => sc.events.recv().await,
                    None => None,
                }
            };
            match evt {
                Some(SidecarEvent::Done { id, tmp, src_bytes, out_bytes, companions }) => {
                    if let Some(src_path) = src_by_id.get(&id) {
                        if let Err(msg) = apply_done(&app_c, &batch_id, &id, src_path, &tmp, src_bytes, out_bytes, move_to_trash, &companions, &batches).await {
                            let _ = app_c.emit("file-error", FileErrorPayload { id: id.clone(), msg });
                        }
                    }
                    pending -= 1;
                }
                Some(SidecarEvent::Error { id: Some(id), msg }) => {
                    let _ = app_c.emit("file-error", FileErrorPayload { id, msg });
                    pending -= 1;
                }
                Some(SidecarEvent::SkippedNoGain { id, src_bytes }) => {
                    let _ = app_c.emit("file-skipped", serde_json::json!({ "id": id, "srcBytes": src_bytes }));
                    pending -= 1;
                }
                Some(SidecarEvent::CompanionError { id, ext, msg }) => {
                    let _ = app_c.emit("companion-error", serde_json::json!({ "id": id, "ext": ext, "msg": msg }));
                }
                Some(SidecarEvent::ParseError { msg, line }) => {
                    eprintln!("sidecar parse error: {msg} line: {line}");
                }
                Some(SidecarEvent::Error { id: None, msg }) => {
                    eprintln!("sidecar error (no id): {msg}");
                }
                None => break,
            }
        }
        batches.complete(&batch_id);
    });
    Ok(())
}

#[tauri::command]
pub async fn undo_last_batch(batches: State<'_, BatchState>) -> Result<UndoReport, String> {
    let Some(batch) = batches.take_last() else {
        return Ok(UndoReport { restored: 0, attempted: 0 });
    };
    let attempted = batch.disposals.len();
    for d in &batch.disposals {
        // Delete the compressed file that replaced the original
        let _ = tokio::fs::remove_file(&d.original_path).await;
        // Delete WebP/AVIF companions
        for cp in &d.companion_paths {
            let _ = tokio::fs::remove_file(cp).await;
        }
    }
    let restored = crate::trash::restore_all(&batch.disposals).map_err(|e| e.to_string())?;
    Ok(UndoReport { restored, attempted })
}

#[tauri::command]
pub async fn open_trash() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(std::path::PathBuf::from(dirs_next::home_dir().ok_or("no home")?).join(".Trash"))
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg("shell:RecycleBinFolder")
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg("trash:///")
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    Err("Unsupported platform".into())
}

use tauri::Window;

#[tauri::command]
pub async fn confirm_close(window: Window) -> Result<(), String> {
    window.close().map_err(|e| e.to_string())
}
