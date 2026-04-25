// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

pub mod batch;
mod commands;
pub mod encoder;
mod sidecar;
mod trash;

use batch::BatchState;
use commands::SidecarState;
use tauri::{Listener, Manager, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(SidecarState::default())
        .manage(BatchState::default())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            handle.clone().listen("sidecar-crashed", move |_| {
                let h = handle.clone();
                tauri::async_runtime::spawn(async move {
                    let st: tauri::State<commands::SidecarState> = h.state();
                    commands::on_sidecar_crashed(st.inner()).await;
                });
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = tauri::Emitter::emit(window, "close-requested", ());
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::compress,
            commands::undo_last_batch,
            commands::open_trash,
            commands::confirm_close,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
