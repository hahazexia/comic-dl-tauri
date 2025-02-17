// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
// use tauri::{Manager, PhysicalPosition, Position};
mod antbyw;
mod db;
mod log_init;
mod mangadex;
pub mod models;
// mod queue_rwlock;
pub mod schema;
mod utils;

use antbyw::{handle_html, CurrentElement, DataWrapper, HandleHtmlRes, Img};
use bytes::Bytes;
use db::{
    create_download_task, create_table, delete_batch_status_not_downloading, delete_download_task,
    find_tasks_by_dl_type_and_url, get_all_download_tasks, get_download_task, init_db,
    update_batch_status, update_download_task_progress, update_download_task_progress_error,
    update_download_task_status,
};
use image::{load_from_memory, ImageFormat};
use log::{error, info};
use log_init::init_log;
use mangadex::handle_mangadex;
use models::{DownloadTask, PartialDownloadTask};
use std::sync::atomic::{AtomicUsize, Ordering};

// use queue_rwlock::QueuedRwLock;
use reqwest;
use serde::Serialize;
use std::collections::HashMap;
use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, RwLock};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use tokio::task::{spawn, AbortHandle, JoinSet};
use tokio::time::{timeout, Duration};
use utils::{clean_string, create_cache_dir, get_second_level_domain, read_from_json, StatusCode};

pub static TASKS: RwLock<Vec<PartialDownloadTask>> = RwLock::new(Vec::new());
pub static APP_HANDLE: LazyLock<RwLock<Option<AppHandle>>> = LazyLock::new(|| RwLock::new(None));
#[derive(Debug, Clone)]
pub struct DownloadResult {
    group_index: usize,
    index: usize,
    error_msg: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadEvent {
    id: i32,
    progress: String,
    count: i32,
    now_count: i32,
    error_vec: String,
    status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StartAllData {
    id: i32,
    status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StartAllRes {
    tasks: Vec<PartialDownloadTask>,
    changed: Vec<StartAllData>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // 初始化日志
            if let Err(e) = init_log() {
                error!("init log error: {}", e);
                app.emit("err_msg_main", format!("init log failed!"))
                    .unwrap();
            };
            // 创建缓存目录
            if let Err(e) = create_cache_dir() {
                error!("create cache dir failed: {}", e.to_string());
                app.emit("err_msg_main", format!("create cache dir failed!"))
                    .unwrap();
            };
            // 初始化数据库
            if let Err(e) = init_db() {
                error!("{}", e.to_string());
                app.emit("err_msg_main", e.to_string()).unwrap();
            } else {
                // 创建表
                if let Err(e) = create_table() {
                    error!("create_table failed: {}", e.to_string());
                    app.emit("err_msg_main", e.to_string()).unwrap();
                } else {
                    // 获取任务列表存入全局变量 TASKS
                    let db_res = get_all_download_tasks();
                    match db_res {
                        Ok(data) => {
                            {
                                let mut tasks_guard = TASKS.write().unwrap();
                                *tasks_guard = data;
                            }
                            {
                                let tasks_guard = TASKS.read().unwrap();
                                info!("Number of tasks: {}", (*tasks_guard).len());
                            }
                        }
                        Err(e) => {
                            error!("get_all_download_tasks failed: {}", e.to_string());
                        }
                    }
                }
            }
            let app_handle = app.handle();
            let mut app_lock = APP_HANDLE.write().unwrap();
            *app_lock = Some(app_handle.clone());
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_tasks,
            add,
            add_new_task,
            delete_tasks,
            start_or_pause,
            start_all,
            delete_all,
            pause_all,
            pause_all_waiting,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn download_single_image(
    id: i32,
    group_index: usize,
    index: usize,
    url: String,
    save_path: String,
    permit: OwnedSemaphorePermit,
    progress: Arc<AtomicUsize>,
    total: i32,
) -> DownloadResult {
    // info!("图片下载开始");
    let current_status = {
        let temp = if let Ok(tasks) = TASKS.try_read() {
            let target = tasks.iter().find(|x| x.id == id).unwrap();
            target.status.clone()
        } else {
            String::from("")
        };
        temp
    };
    if current_status == "stopped" {
        return DownloadResult {
            group_index,
            index,
            error_msg: String::from("stopped"),
        };
    }
    let mut count = 0;
    let mut res;

    loop {
        count += 1;
        let response_result = timeout(Duration::from_secs(10), reqwest::get(url.clone())).await;

        match response_result {
            Ok(Ok(response)) => {
                if response.status().is_success() {
                    let res_temp = response.bytes().await;

                    match res_temp {
                        Ok(bytes) => {
                            res = bytes;
                        }
                        Err(_e) => {
                            res = Bytes::from("");
                            error!("download_single_image res error: {}", _e);
                        }
                    }
                    break;
                } else {
                    error!("download_single_image response status failed id: {}", id);
                    res = Bytes::from("");
                }
            }
            Ok(Err(_e)) => {
                error!("download_single_image id: {} err: {}", id, _e);
                res = Bytes::from("");
            }
            Err(e) => {
                error!("download_single_image id: {} err: {}", id, e);
                res = Bytes::from("");
            }
        }

        if count > 10 {
            break;
        }
    }

    let mut error_msg = String::from("");
    if res.is_empty() {
        error_msg = format!(
            "download img failed: id: {} save_path: {} group_index: {} index: {}",
            id, &save_path, group_index, index
        );
    } else {
        // 处理图片格式
        if let Ok(img) = load_from_memory(&res) {
            let jpg_bytes = img.to_rgb8();
            if let Ok(mut output_file) = File::create(PathBuf::from(&save_path)) {
                if let Err(e) = jpg_bytes.write_to(&mut output_file, ImageFormat::Jpeg) {
                    error_msg = format!(
                        "Failed to write image to file for id: {} save_path: {} group: {} index: {} e: {}",
                        id, &save_path, group_index, index, e
                    );
                }
            } else {
                error_msg = format!(
                    "Failed to create file for id: {} save_path: {} group: {} index: {}",
                    id, &save_path, group_index, index
                );
            }
        } else {
            error_msg = format!(
                "Failed to load image from memory for id: {} save_path: {} group: {} index: {}",
                id, &save_path, group_index, index
            );
        }

        // 更新进度
        if error_msg.is_empty() {
            progress.fetch_add(1, Ordering::Relaxed);
            let pro = progress.load(Ordering::Relaxed);
            if pro % 10 == 0 {
                let app_lock = APP_HANDLE.read().unwrap().clone();

                let progress_str = format!("{:.2}", ((pro as f32) / (total as f32) * 100.00));
                let current_status = {
                    let temp = if let Ok(tasks) = TASKS.try_read() {
                        let target = tasks.iter().find(|x| x.id == id).unwrap();
                        target.status.clone()
                    } else {
                        String::from("downloading")
                    };
                    temp
                };
                if let Some(app) = app_lock {
                    let _ = &app
                        .emit(
                            "progress",
                            DownloadEvent {
                                id: id,
                                progress: progress_str.clone(),
                                count: total,
                                now_count: pro as i32,
                                error_vec: String::from(""),
                                status: current_status,
                            },
                        )
                        .unwrap();
                }
            }
        }
    }
    drop(permit);
    info!("图片下载结束");
    DownloadResult {
        group_index,
        index,
        error_msg,
    }
}

fn sort_tasks() {
    const STATUS_ORDER: [&str; 5] = ["downloading", "waiting", "stopped", "failed", "finished"];
    let mut tasks = TASKS.write().unwrap();
    tasks.sort_by_key(|x| {
        STATUS_ORDER
            .iter()
            .position(|&s| s == x.status.as_str())
            .unwrap_or(STATUS_ORDER.len())
    });
}

fn get_downloading_count() -> i32 {
    let tasks = TASKS.read().unwrap();
    let mut count = 0;
    for task in tasks.iter() {
        if task.status == "downloading" {
            count += 1;
        }
    }
    count
}

fn start_waiting(app: &AppHandle) {
    let current_downloading = get_downloading_count();
    if current_downloading < 2 {
        // 改变第一个 waiting 的 task
        // let mut tasks = TASKS.write().unwrap();
        // if let Some(task) = tasks.iter_mut().find(|t| t.status == "waiting") {
        //     task.status = "downloading".to_string();
        // }
        let change_count = 2 - current_downloading;
        let mut modified_count = 0;
        let tasks = TASKS.write().unwrap();
        let mut changed_vec: Vec<i32> = Vec::new();
        for task in tasks.iter() {
            if task.status == "waiting" {
                info!("!!!!!!!!!! will start {:?}", &task);
                changed_vec.push(task.id);
                // task.status = "downloading".to_string();
                modified_count += 1;
                if modified_count == change_count {
                    break;
                }
            }
        }
        app.emit("start_waiting", changed_vec).unwrap();
    }
}

async fn run_join_set_juanhuafanwai(complete_current_task: DownloadTask) {
    let all_count = complete_current_task.count;
    let cache_json_str = &complete_current_task.cache_json;
    let cache_json: Vec<CurrentElement> = serde_json::from_str(&cache_json_str).unwrap();
    let total = all_count;
    let progress = Arc::new(AtomicUsize::new(0));

    let home_dir = home::home_dir().unwrap();
    let comic_basic_path = home_dir.join(format!(".comic_dl_tauri/download/"));
    let mut all_results = Vec::new();
    let mut cache_json_sync = cache_json.clone();

    'outer: for (group_index, url_group) in cache_json.into_iter().enumerate() {
        let mut group_handles = Vec::<AbortHandle>::new();
        let mut join_set = JoinSet::new();

        let semaphore = Arc::new(Semaphore::new(10));

        for (i, url) in url_group.imgs.into_iter().enumerate() {
            if url.done {
                progress.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            let comic_type = match complete_current_task.dl_type.as_str() {
                "juan" => String::from("单行本"),
                "hua" => String::from("单话"),
                "fanwai" => String::from("番外篇"),
                "current" => String::from("current"),
                _ => String::from(""),
            };
            let save_path_temp = if complete_current_task.author.is_empty() {
                comic_basic_path.join(format!(
                    "{}_{}/{}/{}.jpg",
                    complete_current_task.comic_name, &comic_type, url_group.name, i,
                ))
            } else {
                comic_basic_path.join(format!(
                    "{}/{}_{}/{}/{}.jpg",
                    complete_current_task.author,
                    complete_current_task.comic_name,
                    &comic_type,
                    url_group.name,
                    i,
                ))
            };
            let parent_path = save_path_temp.parent().unwrap();
            if !parent_path.exists() {
                fs::create_dir_all(parent_path).unwrap();
            }
            let save_path = save_path_temp.to_str().unwrap().to_string().clone();
            let url_str = url.href.clone();
            let process_clone = progress.clone();
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let handle = join_set.spawn(download_single_image(
                complete_current_task.id,
                group_index,
                i,
                url_str,
                save_path,
                permit,
                process_clone,
                total,
            ));
            group_handles.push(handle);
        }

        while let Some(res) = join_set.join_next().await {
            info!("join set 任务完成");
            if let Ok(result) = res {
                if result.error_msg == "stopped" {
                    for handle in group_handles.iter() {
                        handle.abort();
                    }
                    break 'outer;
                }
                all_results.push(result.clone());
                if !&result.error_msg.is_empty() {
                    error!("Task error: {}", &result.error_msg);
                } else {
                    cache_json_sync[result.group_index].imgs[result.index].done = true;
                }
            } else {
                error!("Task join error");
                for handle in group_handles.iter() {
                    handle.abort();
                }
                break 'outer;
            }

            let current_progress = progress.load(Ordering::Relaxed);
            if current_progress % 10 == 0 {
                let progress_str = format!(
                    "{:.2}",
                    ((current_progress as f32) / (total as f32) * 100.00)
                );

                if let Err(e) = update_download_task_progress(
                    complete_current_task.id,
                    &progress_str,
                    current_progress as i32,
                    &serde_json::to_string_pretty(&cache_json_sync).unwrap(),
                ) {
                    error!("save progress to db failed: {}", e);
                }

                {
                    if let Ok(mut tasks) = TASKS.try_write() {
                        if let Some(temp) =
                            tasks.iter_mut().find(|x| x.id == complete_current_task.id)
                        {
                            temp.progress = progress_str;
                            temp.now_count = current_progress as i32;
                        }
                    } else {
                        error!("Failed to sync TASKS");
                    }
                }
                sort_tasks();
            }
        }
    }
    // 确保最后一次进度也保存到数据库
    let current_progress = progress.load(Ordering::Relaxed);
    let progress_str = format!(
        "{:.2}",
        ((current_progress as f32) / (total as f32) * 100.00)
    );

    // 可以在这里进一步处理所有的下载结果 all_results
    let mut error_vec: Vec<String> = Vec::new();
    for result in all_results {
        if !result.error_msg.is_empty() && result.error_msg != "stopped" {
            error_vec.push(result.error_msg);
        }
    }

    let status_for_db;
    if !error_vec.is_empty() {
        error!("Final result error: {:?}", &error_vec);

        status_for_db = "failed";
    } else {
        status_for_db = if current_progress as i32 == total {
            "finished"
        } else {
            "stopped"
        };
    }
    let app_lock = APP_HANDLE.read().unwrap().clone();
    if let Some(app) = app_lock {
        let _ = &app
            .emit(
                "progress",
                DownloadEvent {
                    id: complete_current_task.id,
                    progress: progress_str.clone(),
                    count: total,
                    now_count: current_progress as i32,
                    error_vec: serde_json::to_string_pretty(&error_vec).unwrap(),
                    status: status_for_db.to_string(),
                },
            )
            .unwrap();
    }
    if let Err(e) = update_download_task_progress_error(
        complete_current_task.id,
        &progress_str,
        current_progress as i32,
        &serde_json::to_string_pretty(&cache_json_sync).unwrap(),
        serde_json::to_string_pretty(&error_vec).unwrap().as_str(),
        status_for_db,
    ) {
        error!("save error_msg to db failed: {}", e);
    }
    {
        let mut tasks = TASKS.write().unwrap();
        if let Some(temp) = tasks.iter_mut().find(|x| x.id == complete_current_task.id) {
            temp.error_vec = serde_json::to_string_pretty(&error_vec).unwrap();
            temp.status = status_for_db.to_string();
            temp.progress = progress_str.clone();
            temp.now_count = current_progress as i32;
        }
    }
    sort_tasks();
}

async fn run_join_set_current(complete_current_task: DownloadTask) {
    let all_count = complete_current_task.count;
    let cache_json_str = &complete_current_task.cache_json;
    let cache_json: Vec<Img> = serde_json::from_str(&cache_json_str).unwrap();
    let semaphore = Arc::new(Semaphore::new(10));
    let total = all_count;
    let progress = Arc::new(AtomicUsize::new(0));
    let mut all_results = Vec::new();
    let mut cache_json_sync = cache_json.clone();
    let mut group_handles = Vec::<AbortHandle>::new();

    let home_dir = home::home_dir().unwrap();
    let comic_basic_path = home_dir.join(format!(".comic_dl_tauri/download/"));

    let mut join_set = JoinSet::new();

    for (i, url) in cache_json.into_iter().enumerate() {
        if url.done {
            progress.fetch_add(1, Ordering::Relaxed);
            continue;
        }
        let save_path_temp =
            comic_basic_path.join(format!("{}/{}.jpg", complete_current_task.comic_name, i,));
        let parent_path = save_path_temp.parent().unwrap();
        if !parent_path.exists() {
            fs::create_dir_all(parent_path).unwrap();
        }
        let save_path = save_path_temp.to_str().unwrap().to_string().clone();
        let url_str = url.href.clone();

        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let progress_clone = progress.clone();
        let handle = join_set.spawn(download_single_image(
            complete_current_task.id,
            0,
            i,
            url_str,
            save_path,
            permit,
            progress_clone,
            total,
        ));
        group_handles.push(handle);
    }

    while let Some(res) = join_set.join_next().await {
        if let Ok(result) = res {
            if result.error_msg == "stopped" {
                for handle in group_handles.iter() {
                    handle.abort();
                }
                break;
            }
            all_results.push(result.clone());
            if !&result.error_msg.is_empty() {
                error!("current Task error: {}", &result.error_msg);
            } else {
                cache_json_sync[result.index].done = true;
            }
        } else {
            error!("current Task join error");
            for handle in group_handles.iter() {
                handle.abort();
            }
            break;
        }

        // 保存进度到数据库
        let current_progress = progress.load(Ordering::Relaxed);
        if current_progress % 10 == 0 {
            let progress_str = format!(
                "{:.2}",
                ((current_progress as f32) / (total as f32) * 100.00)
            );

            if let Err(e) = update_download_task_progress(
                complete_current_task.id,
                &progress_str,
                current_progress as i32,
                &serde_json::to_string_pretty(&cache_json_sync).unwrap(),
            ) {
                error!("save current progress to db failed: {}", e);
            }
            {
                if let Ok(mut tasks) = TASKS.try_write() {
                    if let Some(temp) = tasks.iter_mut().find(|x| x.id == complete_current_task.id)
                    {
                        temp.progress = progress_str;
                        temp.now_count = current_progress as i32;
                    }
                } else {
                    error!("Failed to sync TASKS");
                }
            }
        }
    }

    // 确保最后一次进度也保存到数据库
    let current_progress = progress.load(Ordering::Relaxed);
    let progress_str = format!(
        "{:.2}",
        ((current_progress as f32) / (total as f32) * 100.00)
    );

    // 可以在这里进一步处理所有的下载结果 all_results
    let mut error_vec: Vec<String> = Vec::new();
    for result in all_results {
        if !result.error_msg.is_empty() && result.error_msg != "stopped" {
            error_vec.push(result.error_msg);
        }
    }

    let status_for_db;
    if !error_vec.is_empty() {
        error!("current Final result error: {:?}", &error_vec);
        status_for_db = "failed";
    } else {
        status_for_db = if current_progress as i32 == total {
            "finished"
        } else {
            "stopped"
        };
    }
    let app_lock = APP_HANDLE.read().unwrap().clone();
    if let Some(app) = app_lock {
        let _ = &app
            .emit(
                "progress",
                DownloadEvent {
                    id: complete_current_task.id,
                    progress: progress_str.clone(),
                    count: total,
                    now_count: current_progress as i32,
                    error_vec: serde_json::to_string_pretty(&error_vec).unwrap(),
                    status: status_for_db.to_string(),
                },
            )
            .unwrap();
    }

    if let Err(e) = update_download_task_progress_error(
        complete_current_task.id,
        &progress_str,
        current_progress as i32,
        &serde_json::to_string_pretty(&cache_json_sync).unwrap(),
        serde_json::to_string_pretty(&error_vec).unwrap().as_str(),
        status_for_db,
    ) {
        error!("save error_msg to db failed: {}", e);
    }
    {
        let mut tasks = TASKS.write().unwrap();
        if let Some(temp) = tasks.iter_mut().find(|x| x.id == complete_current_task.id) {
            temp.error_vec = serde_json::to_string_pretty(&error_vec).unwrap();
            temp.status = status_for_db.to_string();
            temp.progress = progress_str.clone();
            temp.now_count = current_progress as i32;
        }
    }
    sort_tasks();
}

#[tauri::command]
async fn start_or_pause(app: AppHandle, id: i32, status: String) {
    if status == "stopped" {
        let mut tasks = TASKS.write().unwrap();
        for task in tasks.iter_mut() {
            if task.id == id {
                task.status = String::from("stopped");
            }
        }

        let _update_res = update_download_task_status(id, &status);
        let _ = &app
            .emit(
                "task_status",
                HashMap::from([("id", id.to_string()), ("status", String::from("stopped"))]),
            )
            .unwrap();
        return;
    }

    let downloading_count = get_downloading_count();
    info!(
        "start_or_pause downloading_count: {} status: {} id: {}",
        downloading_count, status, id
    );
    let final_status = if downloading_count >= 2 && status == "downloading" {
        String::from("waiting")
    } else {
        String::from("downloading")
    };
    let current_task = {
        let mut tasks = TASKS.write().unwrap();
        let mut current_task = None;
        for task in tasks.iter_mut() {
            if task.id == id {
                task.status = final_status.clone();
                current_task = Some(task.clone());
            }
        }
        current_task
    };

    sort_tasks();

    if let Some(current_task_temp) = current_task {
        let update_res = update_download_task_status(id, &final_status);
        if update_res.is_ok() {
            info!("update task status success: {}", id);
            let _ = &app
                .emit(
                    "task_status",
                    HashMap::from([("id", id.to_string()), ("status", final_status.clone())]),
                )
                .unwrap();
            if final_status == "downloading" {
                let complete_current_task: DownloadTask = get_download_task(id).unwrap();
                let complete_current_task_copy = complete_current_task.clone();
                if current_task_temp.dl_type == "juan"
                    || current_task_temp.dl_type == "hua"
                    || current_task_temp.dl_type == "fanwai"
                {
                    let thread_error_msg = format!(
                        "The child thread crashed: id: {} comic_name: {} dl_type: {}",
                        &complete_current_task.id,
                        &complete_current_task.comic_name,
                        &complete_current_task.dl_type,
                    );

                    let result = spawn(run_join_set_juanhuafanwai(complete_current_task));
                    if let Err(e) = result.await {
                        error!("thread_error_msg: {} e: {}", &thread_error_msg, e);

                        let app_lock = APP_HANDLE.read().unwrap().clone();
                        if let Some(app) = app_lock {
                            let _ = &app.emit("err_msg_main", &thread_error_msg).unwrap();
                        }
                    } else {
                        info!("id: {} thread finished", id);
                        let app_lock = APP_HANDLE.read().unwrap().clone();
                        if let Some(app) = app_lock {
                            start_waiting(&app);
                        }
                    }
                } else if current_task_temp.dl_type == "current" {
                    let thread_error_msg = format!(
                        "The child thread crashed: id: {} comic_name: {} dl_type: {}",
                        &current_task_temp.id,
                        &current_task_temp.comic_name,
                        &current_task_temp.dl_type,
                    );

                    let result = spawn(run_join_set_current(complete_current_task_copy));
                    if let Err(e) = result.await {
                        error!("thread_error_msg: {} e: {}", &thread_error_msg, e);
                        let app_lock = APP_HANDLE.read().unwrap().clone();
                        if let Some(app) = app_lock {
                            let _ = &app.emit("err_msg_main", &thread_error_msg).unwrap();
                        }
                    } else {
                        info!("id: {} thread finished", id);
                        let app_lock = APP_HANDLE.read().unwrap().clone();
                        if let Some(app) = app_lock {
                            start_waiting(&app);
                        }
                    }
                }
            }
        } else {
            error!(
                "update task status failed: {}, status: {}",
                id, &final_status
            );
            let _ = &app
                .emit("info_msg_main", "update task status failed")
                .unwrap();
        }
    }
}

#[tauri::command]
async fn get_tasks(_app: AppHandle) -> Vec<PartialDownloadTask> {
    info!("get_tasks");
    let tasks = TASKS.read().unwrap().clone();
    tasks
}

#[tauri::command]
async fn start_all(_app: AppHandle) -> StartAllRes {
    info!("start_all ");
    let mut tasks = TASKS.write().unwrap();
    let mut count = 0;
    let mut data_for_db: Vec<StartAllData> = Vec::new();
    for task in tasks.iter_mut() {
        if task.status == "downloading" {
            count += 1;
            continue;
        } else if task.status == "stopped" || task.status == "failed" {
            info!(
                "start_all status: {} id: {} count: {}",
                task.status, task.id, count
            );
            if count >= 2 {
                task.status = String::from("waiting");
                data_for_db.push(StartAllData {
                    id: task.id,
                    status: String::from("waiting"),
                });
            } else {
                // task.status = String::from("downloading");
                data_for_db.push(StartAllData {
                    id: task.id,
                    status: String::from("downloading"),
                });
                count += 1;
            }
        }
    }
    info!("data_for_db: {:?}", &data_for_db);
    update_batch_status(&data_for_db);

    let tasks_res = tasks.clone();
    StartAllRes {
        tasks: tasks_res,
        changed: data_for_db,
    }
}

#[tauri::command]
async fn delete_all() -> Vec<PartialDownloadTask> {
    let mut tasks = TASKS.write().unwrap();
    let mut data_for_db: Vec<i32> = Vec::new();
    let mut new_tasks: Vec<PartialDownloadTask> = Vec::new();
    for task in tasks.drain(..) {
        if task.status != "downloading" {
            data_for_db.push(task.id);
        } else {
            new_tasks.push(task);
        }
    }
    *tasks = new_tasks;

    let _ = delete_batch_status_not_downloading(data_for_db);

    tasks.clone()
}

#[tauri::command]
async fn delete_tasks(_app: AppHandle, id: i32) -> isize {
    let del_res = delete_download_task(id);
    match del_res {
        Ok(res) => {
            let mut tasks = TASKS.write().unwrap();
            tasks.retain(|x| x.id != id);
            info!("delete task: {}", res);
            res as isize
        }
        Err(e) => {
            error!("delete task failed: {} e: {}", id, e);
            -1
        }
    }
}

#[tauri::command]
async fn pause_all(_app: AppHandle) -> Vec<PartialDownloadTask> {
    let tasks = loop {
        match TASKS.try_write() {
            Ok(mut tasks) => {
                let mut data_for_db: Vec<StartAllData> = Vec::new();
                for task in tasks.iter_mut() {
                    if task.status == "downloading" || task.status == "waiting" {
                        task.status = String::from("stopped");
                        data_for_db.push(StartAllData {
                            id: task.id,
                            status: String::from("stopped"),
                        });
                    }
                }
                update_batch_status(&data_for_db);
                break tasks;
            }
            Err(_e) => {}
        };
    };

    tasks.clone()
}

#[tauri::command]
async fn pause_all_waiting() -> Vec<PartialDownloadTask> {
    let tasks = loop {
        match TASKS.try_write() {
            Ok(mut tasks) => {
                let mut data_for_db: Vec<StartAllData> = Vec::new();
                for task in tasks.iter_mut() {
                    if task.status == "waiting" {
                        task.status = String::from("stopped");
                        data_for_db.push(StartAllData {
                            id: task.id,
                            status: String::from("stopped"),
                        });
                    }
                }
                update_batch_status(&data_for_db);
                break tasks;
            }
            Err(_e) => {}
        }
    };

    tasks.clone()
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
        app.emit("err_msg_add", format!("url is invalid!")).unwrap();
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
                                app.emit_to("main", "info_msg_main", "already has this task!")
                                    .unwrap();
                                app.emit_to("main", "info_msg_main", "already has this task!")
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
                                            res.comic_name + "_" + &res.current_name;
                                        let db_res = create_download_task(
                                            &dl_type_temp,
                                            "stopped",
                                            &res.local,
                                            &current_data_json,
                                            &url,
                                            &clean_string(&res.author),
                                            &clean_string(&current_name),
                                            "0.00",
                                            res.current_count as i32,
                                            0 as i32,
                                            "",
                                            false,
                                        );
                                        match db_res {
                                            Ok(task) => {
                                                let temp_task = PartialDownloadTask {
                                                    id: task.id,
                                                    dl_type: task.dl_type,
                                                    status: task.status,
                                                    local_path: task.local_path,
                                                    url: task.url,
                                                    author: task.author,
                                                    comic_name: task.comic_name,
                                                    progress: task.progress,
                                                    count: task.count,
                                                    now_count: task.now_count,
                                                    error_vec: task.error_vec,
                                                    done: task.done,
                                                };

                                                let tasks_to_log = {
                                                    let mut tasks = TASKS.write().unwrap();
                                                    tasks.push(temp_task.clone());
                                                    (*tasks).clone()
                                                };
                                                sort_tasks();
                                                app.emit("new_task", &temp_task).unwrap();
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
                                                    "err_msg_main",
                                                    "insert current task failed!",
                                                )
                                                .unwrap();
                                            }
                                        }
                                    }
                                } else {
                                    app.emit("err_msg_add", "handle current html failed!")
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
                                    app.emit("err_msg_add", "handle juan_hua_fanwai html failed!")
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
                                    app.emit("err_msg_add", "handle juan_hua_fanwai html failed!")
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
                                    app.emit("err_msg_add", "handle juan_hua_fanwai html failed!")
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
                                    app.emit("err_msg_add", "handle juan_hua_fanwai html failed!")
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
                                                        data.url.clone(),
                                                    );
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    app.emit("err_msg_add", "handle author html failed!")
                                        .unwrap();
                                }
                            }
                            _ => {}
                        }
                    }
                    let add_window = app.get_webview_window("add").unwrap();
                    let _ = add_window.close();
                }
                "mangadex" => {
                    let _ = handle_mangadex(url.clone()).await;
                }
                _ => {
                    app.emit("err_msg_add", "unknown manga site, not support")
                        .unwrap();
                }
            }
        } else {
            app.emit("err_msg_add", "unknown manga site, not support")
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
    let dl_type_divide = match key.as_str() {
        "单行本" => "juan",
        "单话" => "hua",
        "番外篇" => "fanwai",
        _ => "",
    };
    let db_task_res = find_tasks_by_dl_type_and_url(dl_type_divide, &url);
    let no_find: bool = match db_task_res {
        Ok(data) => {
            let res = if data.is_empty() {
                true
            } else {
                info!(
                    "already has this task: dl_type_divide: {} url: {}",
                    &dl_type_divide, &url
                );
                // info!("add_new_task_juan_hua_fanwai alread has: {:?}", &data);
                app.emit_to("main", "info_msg_main", "already has this task!")
                    .unwrap();
                app.emit_to("main", "info_msg_add", "already has this task!")
                    .unwrap();
                false
            };
            res
        }
        Err(_e) => {
            error!("find_tasks_by_dl_type_and_url failed: {}", _e.to_string());
            true
        }
    };
    if no_find {
        let mut all_count: i32 = 0;
        let new_value = value
            .clone()
            .iter()
            .map(|x| {
                all_count += x.count as i32;
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
        info!("dl_type_divide: {}", dl_type_divide);
        let db_res = create_download_task(
            dl_type_divide,
            "stopped",
            &res.local,
            &data_json,
            &url,
            &clean_string(&res.author),
            &clean_string(&res.comic_name),
            "0.00",
            all_count,
            0 as i32,
            "",
            false,
        );
        match db_res {
            Ok(task) => {
                let temp_task = PartialDownloadTask {
                    id: task.id,
                    dl_type: task.dl_type,
                    status: task.status,
                    local_path: task.local_path,
                    url: task.url,
                    author: task.author,
                    comic_name: task.comic_name,
                    progress: task.progress,
                    count: all_count,
                    now_count: task.now_count,
                    error_vec: task.error_vec,
                    done: task.done,
                };

                let tasks_to_log = {
                    let mut tasks = TASKS.write().unwrap();
                    tasks.push(temp_task.clone());
                    (*tasks).clone()
                };
                sort_tasks();

                app.emit("new_task", &temp_task).unwrap();
                info!(
                    "{} tasks:  {}",
                    dl_type_divide,
                    serde_json::to_string_pretty(&tasks_to_log).unwrap()
                );
            }
            Err(e) => {
                error!("insert juan task failed: {}", e.to_string());
                app.emit("err_msg_main", "insert juan task failed!")
                    .unwrap();
            }
        }
    }
}
