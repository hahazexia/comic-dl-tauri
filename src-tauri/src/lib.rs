// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
// use tauri::{Manager, PhysicalPosition, Position};
mod antbyw;
mod log_init;
mod mangadex;
mod utils;

use antbyw::{handle_html, HandleHtmlRes};
use log::{error, info};
use log_init::init_log;
use mangadex::handle_mangadex;
use tauri::{AppHandle, Emitter};
use utils::{create_cache_dir, get_second_level_domain};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if let Err(e) = init_log() {
                error!("init log error: {}", e);
                app.emit("err-msg", format!("init log failed!")).unwrap();
            };
            if let Err(e) = create_cache_dir() {
                error!("create cache dir failed: {}", e.to_string());
                app.emit("err-msg", format!("create cache dir failed!"))
                    .unwrap();
            };
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![add, add_new_task])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// WindowConfig https://docs.rs/tauri-utils/latest/tauri_utils/config/struct.WindowConfig.html
#[tauri::command]
async fn add(app: AppHandle) {
    info!("open add task window");
    let config = tauri_utils::config::WindowConfig {
        label: "add".to_string(),
        create: false,
        url: tauri::WebviewUrl::App("add.html".into()),
        user_agent: None,
        drag_drop_enabled: true,
        center: true,
        x: None,
        y: None,
        width: 800_f64,
        height: 200_f64,
        min_width: None,
        min_height: None,
        max_width: None,
        max_height: None,
        resizable: true,
        maximizable: true,
        minimizable: true,
        closable: true,
        title: "Add new task".to_string(),
        fullscreen: false,
        focus: false,
        transparent: false,
        maximized: false,
        visible: true,
        decorations: true,
        always_on_bottom: false,
        always_on_top: true,
        visible_on_all_workspaces: false,
        content_protected: false,
        skip_taskbar: false,
        window_classname: None,
        theme: None,
        title_bar_style: Default::default(),
        hidden_title: false,
        accept_first_mouse: false,
        tabbing_identifier: None,
        additional_browser_args: None,
        shadow: true,
        window_effects: None,
        incognito: false,
        parent: None,
        proxy_url: None,
        zoom_hotkeys_enabled: false,
        browser_extensions_enabled: false,
        use_https_scheme: false,
        devtools: None,
        background_color: None,
    };
    let _webview_window = tauri::WebviewWindowBuilder::from_config(&app, &config)
        .unwrap()
        .build()
        .unwrap();
}

// #[tauri::command]
// fn set_add_position(app: AppHandle, x: i32, y: i32) {
//     let add_window = app.get_webview_window("add").unwrap();
//     add_window.set_position(Position::Physical(PhysicalPosition { x: x, y: y })).unwrap();
// }

#[tauri::command]
async fn add_new_task(app: AppHandle, url: String, dl_type: String) {
    info!("add_new_task url: {}, type: {}", &url, &dl_type);

    let dl_type_temp = dl_type.as_str();
    if url.is_empty()
        || (!url.starts_with("https://www.antbyw.com/")
            && !url.starts_with("https://mangadex.org/"))
    {
        app.emit("err-msg", format!("url is invalid!")).unwrap();
    } else {
        let site_name_temp = get_second_level_domain(&url);
        if let Some(site_name) = site_name_temp {
            match site_name.as_str() {
                "antbyw" => {
                    let res: HandleHtmlRes = handle_html(url.clone(), dl_type.clone(), &app).await;
                    info!("{:?}", res.code());
                    match dl_type_temp {
                        "author" => {}
                        "current" => {}
                        "juan" => {}
                        "hua" => {}
                        "fanwai" => {}
                        "juan_hua_fanwai" => {}
                        _ => {}
                    }
                }
                "mangadex" => {
                    let _ = handle_mangadex(url.clone()).await;
                }
                _ => {
                    app.emit("err-msg", "unknown manga site, not support")
                        .unwrap();
                }
            }
        } else {
            app.emit("err-msg", "unknown manga site, not support")
                .unwrap();
        }
    }
}
