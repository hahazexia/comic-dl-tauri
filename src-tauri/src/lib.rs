// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
// use tauri::{Manager, PhysicalPosition, Position};
mod antbyw;
mod db;
mod log_init;
mod mangadex;
pub mod models;
pub mod schema;
mod utils;

use antbyw::{handle_html, CurrentElement, DataWrapper, HandleHtmlRes};
use db::{
    create_download_task, create_table, find_tasks_by_dl_type_and_url, get_all_download_tasks,
    init_db,
};
use log::{error, info};
use log_init::init_log;
use mangadex::handle_mangadex;
use models::PartialDownloadTask;
use std::sync::RwLock;
use tauri::{AppHandle, Emitter};
use utils::{create_cache_dir, get_second_level_domain, read_from_json, StatusCode};

pub static TASKS: RwLock<Vec<PartialDownloadTask>> = RwLock::new(Vec::new());

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // 初始化日志
            if let Err(e) = init_log() {
                error!("init log error: {}", e);
                app.emit("err-msg-main", format!("init log failed!"))
                    .unwrap();
            };
            // 创建缓存目录
            if let Err(e) = create_cache_dir() {
                error!("create cache dir failed: {}", e.to_string());
                app.emit("err-msg-main", format!("create cache dir failed!"))
                    .unwrap();
            };
            // 初始化数据库
            if let Err(e) = init_db() {
                error!("{}", e.to_string());
                app.emit("err-msg-main", e.to_string()).unwrap();
            } else {
                // 创建表
                if let Err(e) = create_table() {
                    error!("create_table failed: {}", e.to_string());
                    app.emit("err-msg-main", e.to_string()).unwrap();
                } else {
                    // 获取任务列表存入全局变量 TASKS
                    let db_res = get_all_download_tasks();
                    match db_res {
                        Ok(data) => {
                            if let Ok(mut tasks_guard) = TASKS.write() {
                                *tasks_guard = data;
                            }
                            if let Ok(tasks_guard) = TASKS.read() {
                                info!("Number of tasks: {}", tasks_guard.len());
                            }
                        }
                        Err(e) => {
                            error!("get_all_download_tasks failed: {}", e.to_string());
                        }
                    }
                }
            }
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![get_tasks, add, add_new_task])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn get_tasks(_app: AppHandle) -> Vec<PartialDownloadTask> {
    info!("get_tasks");
    let tasks = TASKS.read().unwrap().clone();
    tasks
}

// WindowConfig https://docs.rs/tauri-utils/latest/tauri_utils/config/struct.WindowConfig.html
#[tauri::command]
fn add(app: AppHandle) {
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
        resizable: false,
        maximizable: false,
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

#[tauri::command]
async fn add_new_task(app: AppHandle, url: String, dl_type: String) {
    info!("add_new_task url: {}, type: {}", &url, &dl_type);

    let dl_type_temp = dl_type.as_str();
    if url.is_empty()
        || (!url.starts_with("https://www.antbyw.com/")
            && !url.starts_with("https://mangadex.org/"))
    {
        app.emit("err-msg-add", format!("url is invalid!")).unwrap();
    } else {
        let site_name_temp = get_second_level_domain(&url);
        if let Some(site_name) = site_name_temp {
            match site_name.as_str() {
                "antbyw" => {
                    let db_task_res = find_tasks_by_dl_type_and_url(&dl_type, &url);
                    let no_find;
                    match db_task_res {
                        Ok(data) => {
                            if data.is_empty() {
                                no_find = true;
                            } else {
                                no_find = false;
                                info!("already has this task!");
                                app.emit_to("main", "info-msg-main", "already has this task!")
                                    .unwrap();
                                app.emit_to("main", "info-msg-add", "already has this task!")
                                    .unwrap();
                            }
                        }
                        Err(_e) => {
                            error!("find_tasks_by_dl_type_and_url failed: {}", _e.to_string());
                            no_find = true;
                        }
                    }

                    if no_find {
                        let res: HandleHtmlRes =
                            handle_html(url.clone(), dl_type.clone(), &app).await;
                        info!("{:?}", res.code());
                        match dl_type_temp {
                            "current" => {
                                if res.code == StatusCode::Success && res.done {
                                    if let DataWrapper::VecData(current_data) = res.data.clone() {
                                        let current_data_json =
                                            serde_json::to_string_pretty(&current_data).unwrap();
                                        let current_name: String =
                                            res.comic_name + &res.current_name;
                                        let db_res = create_download_task(
                                            &dl_type_temp,
                                            "stopped",
                                            &res.local,
                                            &current_data_json,
                                            &url,
                                            &res.author,
                                            &current_name,
                                            "0.00%",
                                            false,
                                        );
                                        match db_res {
                                            Ok(task) => {
                                                let mut tasks = TASKS.write().unwrap();
                                                let temp_task = PartialDownloadTask {
                                                    id: task.id,
                                                    dl_type: task.dl_type,
                                                    status: task.status,
                                                    local_path: task.local_path,
                                                    url: task.url,
                                                    author: task.author,
                                                    comic_name: task.comic_name,
                                                    progress: task.progress,
                                                    done: task.done,
                                                };

                                                tasks.push(temp_task.clone());
                                                let tasks_to_log = (*tasks).clone();
                                                app.emit("new-task", &temp_task).unwrap();
                                                info!(
                                                    "current tasks:  {}",
                                                    serde_json::to_string_pretty(&tasks_to_log)
                                                        .unwrap()
                                                );
                                            }
                                            Err(e) => {
                                                error!(
                                                    "insert current task failed: {}",
                                                    e.to_string()
                                                );
                                                app.emit(
                                                    "err-msg-main",
                                                    "insert current task failed!",
                                                )
                                                .unwrap();
                                            }
                                        }
                                    }
                                } else {
                                    app.emit("err-msg-add", "handle current html failed!")
                                        .unwrap();
                                }
                            }
                            "juan" => {
                                if res.code == StatusCode::Success && res.done {
                                    if let DataWrapper::HashMapData(juan_hua_fanwai_data) =
                                        res.data.clone()
                                    {
                                        for (key, value) in juan_hua_fanwai_data.iter() {
                                            if key == "单行本" {
                                                add_new_task_juan_hua_fanwai(
                                                    key.to_string(),
                                                    value,
                                                    &res,
                                                    &app,
                                                    url.clone(),
                                                );
                                            }
                                        }
                                    }
                                } else {
                                    app.emit("err-msg-add", "handle juan_hua_fanwai html failed!")
                                        .unwrap();
                                }
                            }
                            "hua" => {
                                if res.code == StatusCode::Success && res.done {
                                    if let DataWrapper::HashMapData(juan_hua_fanwai_data) =
                                        res.data.clone()
                                    {
                                        for (key, value) in juan_hua_fanwai_data.iter() {
                                            if key == "单话" {
                                                add_new_task_juan_hua_fanwai(
                                                    key.to_string(),
                                                    value,
                                                    &res,
                                                    &app,
                                                    url.clone(),
                                                );
                                            }
                                        }
                                    }
                                } else {
                                    app.emit("err-msg-add", "handle juan_hua_fanwai html failed!")
                                        .unwrap();
                                }
                            }
                            "fanwai" => {
                                if res.code == StatusCode::Success && res.done {
                                    if let DataWrapper::HashMapData(juan_hua_fanwai_data) =
                                        res.data.clone()
                                    {
                                        for (key, value) in juan_hua_fanwai_data.iter() {
                                            if key == "番外篇" {
                                                add_new_task_juan_hua_fanwai(
                                                    key.to_string(),
                                                    value,
                                                    &res,
                                                    &app,
                                                    url.clone(),
                                                );
                                            }
                                        }
                                    }
                                } else {
                                    app.emit("err-msg-add", "handle juan_hua_fanwai html failed!")
                                        .unwrap();
                                }
                            }
                            "juan_hua_fanwai" => {
                                if res.code == StatusCode::Success && res.done {
                                    if let DataWrapper::HashMapData(juan_hua_fanwai_data) =
                                        res.data.clone()
                                    {
                                        for (key, value) in juan_hua_fanwai_data.iter() {
                                            add_new_task_juan_hua_fanwai(
                                                key.to_string(),
                                                value,
                                                &res,
                                                &app,
                                                url.clone(),
                                            );
                                        }
                                    }
                                } else {
                                    app.emit("err-msg-add", "handle juan_hua_fanwai html failed!")
                                        .unwrap();
                                }
                            }
                            "author" => {
                                if res.code == StatusCode::Success && res.done {
                                    if let DataWrapper::VecAuthorData(author_data) =
                                        res.data.clone()
                                    {
                                        for data in author_data.iter() {
                                            let comic_json =
                                                read_from_json::<HandleHtmlRes>(&data.local)
                                                    .unwrap();
                                            if let DataWrapper::HashMapData(juan_hua_fanwai_data) =
                                                comic_json.data.clone()
                                            {
                                                for (key, value) in juan_hua_fanwai_data.iter() {
                                                    add_new_task_juan_hua_fanwai(
                                                        key.to_string(),
                                                        value,
                                                        &comic_json,
                                                        &app,
                                                        url.clone(),
                                                    );
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    app.emit("err-msg-add", "handle author html failed!")
                                        .unwrap();
                                }
                            }
                            _ => {}
                        }
                    }
                }
                "mangadex" => {
                    let _ = handle_mangadex(url.clone()).await;
                }
                _ => {
                    app.emit("err-msg-add", "unknown manga site, not support")
                        .unwrap();
                }
            }
        } else {
            app.emit("err-msg-add", "unknown manga site, not support")
                .unwrap();
        }
    }
}

pub fn add_new_task_juan_hua_fanwai(
    key: String,
    value: &Vec<CurrentElement>,
    res: &HandleHtmlRes,
    app: &AppHandle,
    url: String,
) {
    let new_value = value
        .clone()
        .iter()
        .map(|x| {
            let temp = CurrentElement {
                name: x.name.clone(),
                href: x.href.clone(),
                imgs: x.imgs.clone(),
                count: x.count,
                done: false,
            };
            temp
        })
        .collect::<Vec<_>>();
    let data_json = serde_json::to_string_pretty(&new_value).unwrap();
    let dl_type_divide = match key.as_str() {
        "单行本" => "juan",
        "单话" => "hua",
        "番外篇" => "fanwai",
        _ => "",
    };
    info!("dl_type_divide: {}", dl_type_divide);
    let db_res = create_download_task(
        dl_type_divide,
        "stopped",
        &res.local,
        &data_json,
        &url,
        &res.author,
        &res.comic_name,
        "0.00%",
        false,
    );
    match db_res {
        Ok(task) => {
            let mut tasks = TASKS.write().unwrap();
            let temp_task = PartialDownloadTask {
                id: task.id,
                dl_type: task.dl_type,
                status: task.status,
                local_path: task.local_path,
                url: task.url,
                author: task.author,
                comic_name: task.comic_name,
                progress: task.progress,
                done: task.done,
            };

            app.emit("new-task", &temp_task).unwrap();
            tasks.push(temp_task.clone());
            let tasks_to_log = (*tasks).clone();
            info!(
                "{} tasks:  {}",
                dl_type_divide,
                serde_json::to_string_pretty(&tasks_to_log).unwrap()
            );
        }
        Err(e) => {
            error!("insert juan task failed: {}", e.to_string());
            app.emit("err-msg-main", "insert juan task failed!")
                .unwrap();
        }
    }
}

// fn cal_task_progress(db_res: &Vec<PartialDownloadTask>) -> Vec<PartialDownloadTask> {
//     let cal_res = db_res.iter().map(|x| {
//         let temp = x.clone();
//     });
// }
