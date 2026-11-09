mod asset;
mod cli;
pub use cli::run as run_cli;
mod commands;
mod model;
mod open_link;
mod parse;
mod store;
mod watch;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(store::StoreState(
            store::DataRoot::open(&store::data_root()),
        ))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .register_asynchronous_uri_scheme_protocol("asset", |context, request, responder| {
            let app = context.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                let response = commands::store_io(app, move |root| {
                    Ok::<_, String>(asset::response(root, request))
                })
                .await
                .unwrap_or_else(|_| {
                    tauri::http::Response::builder()
                        .status(500)
                        .body(Vec::<u8>::new())
                        .unwrap()
                });
                responder.respond(response);
            });
        })
        .setup(|app| {
            if let Ok(root) = app.state::<store::StoreState>().get() {
                watch::spawn(app.handle().clone(), root.path().to_path_buf());
            }
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
            commands::set_repo_pinned,
            commands::print_view,
            open_link::open_local_path,
            open_link::open_note,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
