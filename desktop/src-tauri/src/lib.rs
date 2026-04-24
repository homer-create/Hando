mod commands;
mod sidecar;

use commands::SidecarState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(SidecarState::default())
        .plugin(tauri_plugin_store::Builder::new().build())
        .invoke_handler(tauri::generate_handler![commands::compress])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
