use crate::sidecar::{EncodeCommand, EncodeOpts, Sidecar, SidecarEvent};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
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

fn sidecar_script_path(app: &AppHandle) -> PathBuf {
    let resource_dir = app.path().resource_dir().ok();
    if let Some(dir) = resource_dir {
        let candidate = dir.join("sidecar/sidecar.js");
        if candidate.exists() { return candidate; }
    }
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    here.join("../../src/sidecar.js").canonicalize().unwrap_or(here)
}

async fn ensure_sidecar(app: &AppHandle, state: &SidecarState) -> Result<(), String> {
    let mut guard = state.0.lock().await;
    if guard.is_some() { return Ok(()); }
    let script = sidecar_script_path(app);
    let sc = Sidecar::spawn(PathBuf::from("node"), script)
        .await
        .map_err(|e| e.to_string())?;
    let app_clone = app.clone();
    *guard = Some(sc);
    // Drain events in a background task. We spawn one per Sidecar instance.
    let st = app.state::<SidecarState>();
    let state_arc: Arc<Mutex<Option<Sidecar>>> = Arc::new(Mutex::new(None));
    let _ = (st, state_arc, app_clone);
    Ok(())
}

#[tauri::command]
pub async fn compress(
    app: AppHandle,
    state: State<'_, SidecarState>,
    args: CompressArgs,
) -> Result<(), String> {
    ensure_sidecar(&app, &state).await?;
    // Send one encode command per file.
    {
        let guard = state.0.lock().await;
        let sc = guard.as_ref().ok_or("sidecar missing")?;
        for f in &args.files {
            let cmd = EncodeCommand {
                cmd: "encode",
                id: f.id.clone(),
                src: f.path.clone(),
                ext: f.ext.clone(),
                opts: args.opts.clone(),
            };
            sc.send(&cmd).await.map_err(|e| e.to_string())?;
        }
    }
    // Drain the receiver in a task; route each event to the frontend.
    let app_c = app.clone();
    let state_arc: Arc<Mutex<Option<Sidecar>>> = state.inner().0.clone().into();
    tokio::spawn(async move {
        loop {
            let evt_opt = {
                let mut guard = state_arc.lock().await;
                match guard.as_mut() {
                    Some(sc) => sc.events.recv().await,
                    None => None,
                }
            };
            match evt_opt {
                Some(SidecarEvent::Done { id, tmp, src_bytes, out_bytes, .. }) => {
                    let _ = app_c.emit("file-done", FileDonePayload { id, tmp, src_bytes, out_bytes });
                }
                Some(SidecarEvent::Error { id, msg }) => {
                    if let Some(id) = id {
                        let _ = app_c.emit("file-error", FileErrorPayload { id, msg });
                    } else {
                        eprintln!("sidecar error (no id): {msg}");
                    }
                }
                Some(SidecarEvent::SkippedNoGain { id, src_bytes }) => {
                    let _ = app_c.emit("file-skipped", serde_json::json!({ "id": id, "srcBytes": src_bytes }));
                }
                Some(SidecarEvent::CompanionError { id, ext, msg }) => {
                    let _ = app_c.emit("companion-error", serde_json::json!({ "id": id, "ext": ext, "msg": msg }));
                }
                Some(SidecarEvent::ParseError { msg, line }) => {
                    eprintln!("sidecar parse error: {msg} line: {line}");
                }
                None => break,
            }
        }
    });
    Ok(())
}
