// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

pub mod batch;
mod commands;
pub mod encoder;
mod sidecar; // still present; deleted in Phase 3
mod trash;

use batch::BatchState;
use tauri::WindowEvent;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(BatchState::default())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
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
