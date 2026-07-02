mod commands;
mod model;
mod parse;
mod store;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_data_root,
            commands::list_repos,
            commands::list_notes,
            commands::read_note,
            commands::write_note,
            commands::create_note,
            commands::delete_note,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
