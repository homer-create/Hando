mod batch;
mod commands;
mod sidecar;
mod trash;

use batch::BatchState;
use commands::SidecarState;
use tauri::WindowEvent;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(SidecarState::default())
        .manage(BatchState::default())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_fs::init())
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
