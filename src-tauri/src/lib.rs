mod asset;
pub mod backend;
mod cli;
pub mod session;
pub use cli::run as run_cli;
#[cfg(all(feature = "desktop", not(windows)))]
mod commands;
#[cfg(all(feature = "desktop", windows))]
#[path = "windows_commands.rs"]
mod commands;
mod model;
#[cfg(all(feature = "desktop", not(windows)))]
mod open_link;
mod parse;
mod paths;
mod store;
mod watch;
#[cfg(all(feature = "desktop", windows))]
mod windows;
#[cfg(feature = "desktop")]
use tauri::Manager;

#[cfg(feature = "desktop")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .register_asynchronous_uri_scheme_protocol("asset", |context, request, responder| {
            let app = context.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                #[cfg(windows)]
                let response =
                    commands::blocking(move || windows::asset_response(&app, request)).await;
                #[cfg(not(windows))]
                let response = commands::store_io(app, move |root| {
                    Ok::<_, String>(asset::response(root, request))
                })
                .await;
                let response = response.unwrap_or_else(|_| {
                    tauri::http::Response::builder()
                        .status(500)
                        .body(Vec::<u8>::new())
                        .unwrap()
                });
                responder.respond(response);
            });
        })
        .setup(|app| {
            #[cfg(windows)]
            {
                let connection = windows::Windows::new(app.handle());
                let config = connection.view()?.config;
                app.manage(connection);
                if let Some(config) = config {
                    let handle = app.handle().clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = commands::blocking(move || {
                            handle
                                .state::<windows::Windows>()
                                .connect(&handle, config, None)
                        })
                        .await;
                    });
                }
            }
            #[cfg(not(windows))]
            if let Ok(root) = app.state::<store::StoreState>().get() {
                use tauri::Emitter;
                let root = root.path().to_path_buf();
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    let notifications = handle.clone();
                    let watcher = watch::spawn(root, move |event| {
                        let result = match event {
                            watch::Event::Changed { repo_keys } => notifications.emit(
                                watch::CHANGED_EVENT,
                                serde_json::json!({"repoKeys":repo_keys}),
                            ),
                            watch::Event::WatchError { message } => notifications
                                .emit(watch::ERROR_EVENT, serde_json::json!({"message":message})),
                        };
                        if let Err(error) = result {
                            eprintln!("変更イベントの送信に失敗: {error}");
                        }
                    });
                    match watcher {
                        Ok(watcher) => {
                            handle.manage(std::sync::Mutex::new(watcher));
                            if let Err(error) = handle
                                .emit(watch::CHANGED_EVENT, serde_json::json!({"repoKeys":[]}))
                            {
                                eprintln!("変更イベントの送信に失敗: {error}");
                            }
                        }
                        Err(error) => eprintln!("{error}"),
                    }
                });
            }
            Ok(())
        });
    #[cfg(not(windows))]
    let builder = builder
        .manage(store::StoreState(
            store::DataRoot::open(&store::data_root()),
        ))
        .invoke_handler(tauri::generate_handler![
            commands::get_connection,
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
        ]);
    #[cfg(windows)]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::get_connection,
        commands::connect_backend,
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
        commands::open_local_path,
        commands::open_note,
    ]);
    builder
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            #[cfg(windows)]
            if matches!(event, tauri::RunEvent::Exit) {
                _app.state::<windows::Windows>().disconnect();
            }
            #[cfg(not(windows))]
            let _ = event;
        });
}
