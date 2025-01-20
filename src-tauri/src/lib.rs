// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, create_window])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
async fn create_window(app: tauri::AppHandle) {
  let webview_window = tauri::WebviewWindowBuilder::new(&app, "label", tauri::WebviewUrl::App("index.html".into()))
    .build()
    .unwrap();
}