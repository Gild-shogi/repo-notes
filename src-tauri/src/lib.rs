mod commands;
mod model;
mod open_link;
mod parse;
mod store;
mod watch;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            watch::spawn(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_data_root,
            commands::list_repos,
            commands::list_notes,
            commands::read_note,
            commands::write_note,
            commands::create_note,
            commands::delete_note,
            commands::set_pinned,
            open_link::open_local_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
