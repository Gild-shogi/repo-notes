mod asset;
pub mod backend;
mod cli;
pub use cli::run as run_cli;
#[cfg(feature = "desktop")]
mod commands;
mod model;
#[cfg(feature = "desktop")]
mod open_link;
mod parse;
mod paths;
mod store;
mod watch;
#[cfg(feature = "desktop")]
use tauri::Manager;

#[cfg(feature = "desktop")]
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
                use tauri::Emitter;
                let handle = app.handle().clone();
                let watcher = watch::spawn(root.path().to_path_buf(), move |event| {
                    let result = match event {
                        watch::Event::Changed { repo_keys } => handle.emit(
                            watch::CHANGED_EVENT,
                            serde_json::json!({"repoKeys":repo_keys}),
                        ),
                        watch::Event::WatchError { message } => {
                            handle.emit(watch::ERROR_EVENT, serde_json::json!({"message":message}))
                        }
                    };
                    if let Err(error) = result {
                        eprintln!("変更イベントの送信に失敗: {error}");
                    }
                });
                match watcher {
                    Ok(watcher) => {
                        app.manage(std::sync::Mutex::new(watcher));
                        if let Err(error) =
                            app.emit(watch::CHANGED_EVENT, serde_json::json!({"repoKeys":[]}))
                        {
                            eprintln!("変更イベントの送信に失敗: {error}");
                        }
                    }
                    Err(error) => eprintln!("{error}"),
                }
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
