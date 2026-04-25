// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::batch::BatchState;
use crate::encoder::{
    self,
    event_sink::{
        CompanionErrorPayload, EventSink, FileDonePayload, FileErrorPayload,
        FileProgressPayload, FileSkippedPayload, TauriEmitter, TrashFallbackPayload,
    },
    EncodeOpts, EncodeOutcome, EncodeRequest, ImageExt,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tokio::sync::Semaphore;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressArgs {
    pub batch_id: String,
    pub files: Vec<CompressFile>,
    pub opts: EncodeOpts,
    pub move_originals_to_trash: bool,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CompressFile {
    pub id: String,
    pub path: String,
    pub ext: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoReport {
    pub restored: usize,
    pub attempted: usize,
}

fn concurrency_limit() -> usize {
    (num_cpus::get().saturating_sub(1)).max(1).min(8)
}

async fn place_file(tmp: &Path, dest: &Path) -> Result<(), String> {
    if tokio::fs::rename(tmp, dest).await.is_err() {
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
    batch_id: &str,
    file: &CompressFile,
    res: encoder::EncodeResult,
    move_to_trash: bool,
    batches: &BatchState,
    sink: &dyn EventSink,
) -> Result<(), String> {
    let src_path = Path::new(&file.path);

    // Capture original size BEFORE any file operations
    let src_bytes = std::fs::metadata(src_path).map(|m| m.len()).unwrap_or(0);

    if move_to_trash {
        let disposal = crate::trash::dispose_original(src_path)
            .map_err(|e| e.to_string())?;
        let kind_note = match &disposal.kind {
            crate::trash::DisposalKind::Trashed => None,
            crate::trash::DisposalKind::RenamedFallback { backup_path } =>
                Some(format!("Trash unavailable; original backed up to {}", backup_path.display())),
        };
        batches.record_disposal(batch_id, disposal);
        if let Some(note) = kind_note {
            sink.emit_trash_fallback(TrashFallbackPayload { id: file.id.clone(), note });
        }
    } else {
        tokio::fs::remove_file(src_path).await.map_err(|e| e.to_string())?;
    }

    place_file(&res.main.tmp_path, src_path).await?;

    // Place companion files alongside the source
    let mut companion_dests: Vec<PathBuf> = vec![];
    for c in &res.companions {
        let stem = src_path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let parent = src_path.parent().unwrap_or_else(|| Path::new("."));
        let dest = parent.join(format!("{stem}{}", c.ext.dotted()));
        match place_file(&c.tmp_path, &dest).await {
            Ok(()) => companion_dests.push(dest),
            Err(e) => sink.emit_companion_error(CompanionErrorPayload {
                id: file.id.clone(),
                ext: c.ext.dotted().trim_start_matches('.').to_string(),
                msg: e,
            }),
        }
    }

    // Report any companion encode errors
    for ce in &res.companion_errors {
        sink.emit_companion_error(CompanionErrorPayload {
            id: file.id.clone(),
            ext: ce.ext.dotted().trim_start_matches('.').to_string(),
            msg: ce.msg.clone(),
        });
    }

    if move_to_trash {
        batches.record_companion_paths(batch_id, companion_dests);
    }

    sink.emit_file_done(FileDonePayload {
        id: file.id.clone(),
        src_bytes,               // original file size (captured before disposal)
        out_bytes: res.main.bytes, // compressed size
    });
    Ok(())
}

#[tauri::command]
pub async fn compress(
    app: AppHandle,
    batches: State<'_, BatchState>,
    args: CompressArgs,
) -> Result<(), String> {
    batches.start(&args.batch_id, args.files.len());
    let semaphore = Arc::new(Semaphore::new(concurrency_limit()));
    let opts = Arc::new(args.opts);
    let sink: Arc<dyn EventSink> = Arc::new(TauriEmitter::new(app.clone()));

    for f in args.files {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let app_c = app.clone();
        let opts_c = opts.clone();
        let sink_c = sink.clone();
        let batch_id = args.batch_id.clone();
        let move_to_trash = args.move_originals_to_trash;

        tokio::spawn(async move {
            let _permit = permit; // Released on drop
            let f_clone = f.clone();
            let opts_inner = opts_c.clone();

            // Clone what the progress callback needs
            let sink_for_progress = sink_c.clone();
            let id_for_progress = f.id.clone();

            let result = tokio::task::spawn_blocking(move || -> Result<encoder::EncodeOutcome, encoder::EncodeError> {
                let ext = ImageExt::from_str(&f_clone.ext)?;
                encoder::encode(EncodeRequest {
                    src_path: Path::new(&f_clone.path),
                    ext,
                    opts: &opts_inner,
                    progress_cb: Some(Box::new(move |pct: u8| {
                        sink_for_progress.emit_file_progress(FileProgressPayload {
                            id: id_for_progress.clone(),
                            pct,
                        });
                    })),
                })
            }).await;

            let batches_state = app_c.state::<BatchState>();

            match result {
                Ok(Ok(EncodeOutcome::Encoded(res))) => {
                    if let Err(msg) = apply_done(&batch_id, &f, res, move_to_trash, &batches_state, &*sink_c).await {
                        sink_c.emit_file_error(FileErrorPayload { id: f.id.clone(), msg });
                    }
                }
                Ok(Ok(EncodeOutcome::SkippedNoGain { src_bytes })) => {
                    sink_c.emit_file_skipped(FileSkippedPayload { id: f.id.clone(), src_bytes });
                }
                Ok(Err(e)) => {
                    sink_c.emit_file_error(FileErrorPayload { id: f.id.clone(), msg: e.to_string() });
                }
                Err(join_err) => {
                    sink_c.emit_file_error(FileErrorPayload {
                        id: f.id.clone(),
                        msg: format!("encoder panic: {join_err}"),
                    });
                }
            }

            batches_state.tick(&batch_id, &*sink_c);
        });
    }

    Ok(())
}

#[tauri::command]
pub async fn undo_last_batch(batches: State<'_, BatchState>) -> Result<UndoReport, String> {
    let Some(batch) = batches.take_last() else {
        return Ok(UndoReport { restored: 0, attempted: 0 });
    };
    let attempted = batch.disposals.len();
    for d in &batch.disposals {
        let _ = tokio::fs::remove_file(&d.original_path).await;
        for cp in &d.companion_paths {
            let _ = tokio::fs::remove_file(cp).await;
        }
    }
    let restored = crate::trash::restore_all(&batch.disposals)
        .map_err(|e| e.to_string())?;
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

#[tauri::command]
pub async fn confirm_close(app: AppHandle) -> Result<(), String> {
    app.exit(0);
    Ok(())
}
