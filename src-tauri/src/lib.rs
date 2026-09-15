mod commands;
mod settings;

use settings::SettingsStore;
use tauri::Manager;

pub use commands::*;

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .filter_module("tao", log::LevelFilter::Warn)
        .filter_module("wry", log::LevelFilter::Warn)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let appdata = settings::app_data_dir();
            let scenarios = settings::scenarios_dir();
            std::fs::create_dir_all(&appdata)?;
            std::fs::create_dir_all(&scenarios)?;

            let store = SettingsStore::new(&appdata);
            app.manage(store);

            // init the python bridge + services (blogger client, cdp chrome)
            blogger_python::bridge_init(&appdata, &scenarios)
                .map_err(|e| tauri::Error::AssetNotFound(e.to_string()))?;

            // ensure the embedded python interpreter is up before the window loads
            blogger_python::ensure_python()
                .map_err(|e| tauri::Error::AssetNotFound(e.to_string()))?;

            log::info!(
                "app data dir: {} | scenarios: {}",
                appdata.display(),
                scenarios.display()
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // settings
            get_settings,
            update_settings,
            // chrome
            chrome_connect,
            chrome_status,
            chrome_launch,
            chrome_list_tabs,
            chrome_close_tab,
            chrome_open_editor,
            chrome_evaluate,
            // image
            upload_image,
            // oauth
            oauth_get_config,
            oauth_set_config,
            oauth_login_start,
            oauth_complete,
            oauth_status,
            oauth_logout,
            // blogger
            blogger_list_blogs,
            blogger_get_blog,
            blogger_tags,
            blogger_add_tag,
            blogger_remove_tag,
            blogger_settings,
            blogger_posts,
            blogger_get_post,
            blogger_create_post,
            blogger_update_post,
            blogger_publish_post,
            blogger_revert_post,
            blogger_delete_post,
            blogger_refresh_cache,
            // python scenarios
            scenarios_list,
            scenario_run,
            python_selftest,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}