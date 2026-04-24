mod batch;
mod commands;
mod sidecar;
mod trash;

use batch::BatchState;
use commands::SidecarState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(SidecarState::default())
        .manage(BatchState::default())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .invoke_handler(tauri::generate_handler![commands::compress, commands::undo_last_batch])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
