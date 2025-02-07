// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
// use tauri::{Manager, PhysicalPosition, Position};
mod antbyw;
mod db;
mod log_init;
mod mangadex;
pub mod models;
pub mod schema;
mod utils;

use antbyw::{handle_html, CurrentElement, DataWrapper, HandleHtmlRes, Img};
use bytes::Bytes;
use db::{
    create_download_task, create_table, delete_download_task, find_tasks_by_dl_type_and_url,
    get_all_download_tasks, get_download_task, init_db, update_download_task_error_vec,
    update_download_task_progress, update_download_task_status,
};
use image::{load_from_memory, ImageFormat};
use log::{error, info};
use log_init::init_log;
use mangadex::handle_mangadex;
use models::{DownloadTask, PartialDownloadTask};
use reqwest;
use serde::Serialize;
use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::{collections::HashMap, sync::RwLock};
use tauri::{ipc::Channel, AppHandle, Emitter, Manager};
use tokio::sync::Semaphore;
use tokio::task::{AbortHandle, JoinSet};
use tokio::time::{timeout, Duration};
use utils::{create_cache_dir, get_second_level_domain, read_from_json, StatusCode};

pub static TASKS: RwLock<Vec<PartialDownloadTask>> = RwLock::new(Vec::new());

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
        .invoke_handler(tauri::generate_handler![
            get_tasks,
            add,
            add_new_task,
            delete_tasks,
            start_or_pause,
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
    semaphore: Arc<Semaphore>,
    progress: Arc<Mutex<u32>>,
) -> DownloadResult {
    let _permit = match semaphore.acquire().await {
        Ok(permit) => permit,
        Err(_) => {
            let error_msg = format!(
                "Failed to acquire semaphore for group {} index {}",
                group_index, index
            );
            return DownloadResult {
                group_index,
                index,
                error_msg,
            };
        }
    };
    let mut count = 0;
    let messages = vec!["请求失败，状态码", "请求错误", "请求超时", "字节转换失败"];
    let mut err_counts = std::collections::HashMap::new();
    let mut res;

    loop {
        let current_status = {
            let tasks = TASKS.read().unwrap();
            let target = tasks.iter().find(|x| x.id == id).unwrap();
            target.status.clone()
        };
        if current_status == "stopped" {
            tokio::time::sleep(Duration::from_secs(1)).await;
            continue;
        }
        count += 1;
        let response_result = timeout(Duration::from_secs(20), reqwest::get(url.clone())).await;

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
                            *err_counts.entry(&messages[3]).or_insert(3) += 1;
                        }
                    }
                    break;
                } else {
                    res = Bytes::from("");
                    *err_counts.entry(&messages[0]).or_insert(0) += 1;
                }
            }
            Ok(Err(_e)) => {
                res = Bytes::from("");
                *err_counts.entry(&messages[1]).or_insert(0) += 1;
            }
            Err(_) => {
                res = Bytes::from("");
                *err_counts.entry(&messages[2]).or_insert(0) += 1;
            }
        }

        if count > 10 {
            break;
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    let mut error_msg = String::from("");
    if res.is_empty() {
        let mut error_str = format!(
            "download img failed : group_index: {} index: {}",
            group_index, index
        );
        for (msg, index) in err_counts {
            error_str.push_str(&format!(" {}: {}", msg, index));
        }
        error_msg = error_str;
    } else {
        // 处理图片格式
        if let Ok(img) = load_from_memory(&res) {
            let jpg_bytes = img.to_rgb8();
            if let Ok(mut output_file) = File::create(PathBuf::from(save_path)) {
                if let Err(e) = jpg_bytes.write_to(&mut output_file, ImageFormat::Jpeg) {
                    error_msg = format!(
                        "Failed to write image to file for group {} index {}: {}",
                        group_index, index, e
                    );
                }
            } else {
                error_msg = format!(
                    "Failed to create file for group {} index {}",
                    group_index, index
                );
            }
        } else {
            error_msg = format!(
                "Failed to load image from memory for group {} index {}",
                group_index, index
            );
        }

        // 更新进度
        if error_msg.is_empty() {
            let mut progress_lock = progress.lock().unwrap();
            *progress_lock += 1;
        }
    }

    DownloadResult {
        group_index,
        index,
        error_msg,
    }
}

#[tauri::command]
fn start_or_pause(app: AppHandle, id: i32, status: String, on_event: Channel<DownloadEvent>) {
    let home_dir = home::home_dir().unwrap();
    let comic_basic_path = home_dir.join(format!(".comic_dl_tauri/download/"));
    let mut tasks = TASKS.write().unwrap();
    let mut current_task = None;
    for task in tasks.iter_mut() {
        if task.id == id {
            current_task = Some(task.clone());
            task.status = status.clone();
        }
    }

    if let Some(current_task_temp) = current_task {
        let update_res = update_download_task_status(id, &status);
        if update_res.is_ok() {
            info!("update task status success: {}", id);
            app.emit(
                "task_status",
                HashMap::from([("id", id.to_string()), ("status", status.clone())]),
            )
            .unwrap();
            if status == "downloading" {
                let complete_current_task: DownloadTask = get_download_task(id).unwrap();
                let all_count = complete_current_task.count;
                let cache_json_str = complete_current_task.cache_json;
                if current_task_temp.dl_type == "juan"
                    || current_task_temp.dl_type == "hua"
                    || current_task_temp.dl_type == "fanwai"
                {
                    let cache_json: Vec<CurrentElement> =
                        serde_json::from_str(&cache_json_str).unwrap();
                    let semaphore = Arc::new(Semaphore::new(20));
                    let total = all_count;
                    let progress = Arc::new(Mutex::new(0));

                    thread::spawn(move || {
                        tokio::runtime::Builder::new_multi_thread()
                            .enable_all()
                            .build()
                            .unwrap()
                            .block_on(async {
                                let mut all_results = Vec::new();
                                let mut cache_json_sync = cache_json.clone();

                                'outer: for (group_index, url_group) in
                                    cache_json.into_iter().enumerate()
                                {
                                    let mut group_handles = Vec::<AbortHandle>::new();
                                    let mut join_set = JoinSet::new();
                                    let mut group_save_counter = 0;

                                    for (i, url) in url_group.imgs.into_iter().enumerate() {
                                        if url.done {
                                            let mut progress_lock = progress.lock().unwrap();
                                            *progress_lock += 1;
                                            continue;
                                        }
                                        let comic_type =
                                            match complete_current_task.dl_type.as_str() {
                                                "juan" => String::from("单行本"),
                                                "hua" => String::from("单话"),
                                                "fanwai" => String::from("番外篇"),
                                                "current" => String::from("current"),
                                                _ => String::from(""),
                                            };
                                        let save_path_temp =
                                            if complete_current_task.author.is_empty() {
                                                comic_basic_path.join(format!(
                                                    "{}_{}/{}/{}.jpg",
                                                    complete_current_task.comic_name,
                                                    &comic_type,
                                                    url_group.name,
                                                    i,
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
                                        let save_path =
                                            save_path_temp.to_str().unwrap().to_string().clone();
                                        let url_str = url.href.clone();
                                        let semaphore = semaphore.clone();
                                        let progress = progress.clone();
                                        let handle = join_set.spawn(download_single_image(
                                            id,
                                            group_index,
                                            i,
                                            url_str,
                                            save_path,
                                            semaphore,
                                            progress,
                                        ));
                                        group_handles.push(handle);
                                    }

                                    while let Some(res) = join_set.join_next().await {
                                        if let Ok(result) = res {
                                            all_results.push(result.clone());
                                            if !&result.error_msg.is_empty() {
                                                error!("Task error: {}", &result.error_msg);
                                            } else {
                                                cache_json_sync[result.group_index].imgs
                                                    [result.index]
                                                    .done = true;
                                            }
                                        } else {
                                            error!("Task join error");
                                            for handle in group_handles.iter() {
                                                handle.abort();
                                            }
                                            break 'outer;
                                        }
                                        group_save_counter += 1;
                                        if group_save_counter % 5 == 0 {
                                            // 保存进度到数据库
                                            let current_progress = *progress.lock().unwrap();

                                            info!("current_progress: {}", current_progress);
                                            let progress_str = format!(
                                                "{:.2}",
                                                ((current_progress as f32) / (total as f32)
                                                    * 100.00)
                                            );

                                            let event_res = on_event.send(DownloadEvent {
                                                id: complete_current_task.id,
                                                progress: progress_str.clone(),
                                                count: total,
                                                now_count: current_progress as i32,
                                                error_vec: String::from(""),
                                                status: String::from("downloading"),
                                            });
                                            match event_res {
                                                Ok(_s) => {}
                                                Err(e) => {
                                                    error!("on_event failed: {}", e);
                                                }
                                            }

                                            if let Err(e) = update_download_task_progress(
                                                complete_current_task.id,
                                                &progress_str,
                                                current_progress as i32,
                                                &serde_json::to_string_pretty(&cache_json_sync)
                                                    .unwrap(),
                                            ) {
                                                error!("save progress to db failed: {}", e);
                                            } else {
                                                let mut tasks = TASKS.write().unwrap();
                                                if let Some(temp) = tasks
                                                    .iter_mut()
                                                    .find(|x| x.id == complete_current_task.id)
                                                {
                                                    temp.progress = progress_str;
                                                    temp.now_count = current_progress as i32;
                                                }
                                            }
                                        }
                                    }
                                }
                                // 确保最后一次进度也保存到数据库
                                let current_progress = *progress.lock().unwrap();
                                let progress_str = format!(
                                    "{:.2}",
                                    ((current_progress as f32) / (total as f32) * 100.00)
                                );

                                on_event
                                    .send(DownloadEvent {
                                        id: complete_current_task.id,
                                        progress: progress_str.clone(),
                                        count: total,
                                        now_count: current_progress as i32,
                                        error_vec: String::from(""),
                                        status: String::from("downloading"),
                                    })
                                    .unwrap();

                                if let Err(e) = update_download_task_progress(
                                    complete_current_task.id,
                                    &progress_str,
                                    current_progress as i32,
                                    &serde_json::to_string_pretty(&cache_json_sync).unwrap(),
                                ) {
                                    error!("save progress to db failed: {}", e);
                                } else {
                                    let mut tasks = TASKS.write().unwrap();
                                    if let Some(temp) =
                                        tasks.iter_mut().find(|x| x.id == complete_current_task.id)
                                    {
                                        temp.progress = progress_str.clone();
                                        temp.now_count = current_progress as i32;
                                    }
                                }

                                // 可以在这里进一步处理所有的下载结果 all_results
                                let mut error_vec: Vec<String> = Vec::new();
                                for result in all_results {
                                    if !result.error_msg.is_empty() {
                                        error_vec.push(result.error_msg);
                                    }
                                }
                                if !error_vec.is_empty() {
                                    error!("Final result error: {:?}", &error_vec);

                                    on_event
                                        .send(DownloadEvent {
                                            id: complete_current_task.id,
                                            progress: progress_str.clone(),
                                            count: total,
                                            now_count: current_progress as i32,
                                            error_vec: serde_json::to_string_pretty(&error_vec)
                                                .unwrap(),
                                            status: String::from("failed"),
                                        })
                                        .unwrap();
                                    if let Err(e) = update_download_task_error_vec(
                                        complete_current_task.id,
                                        serde_json::to_string_pretty(&error_vec).unwrap().as_str(),
                                        "failed",
                                    ) {
                                        error!("save error_msg to db failed: {}", e);
                                    } else {
                                        let mut tasks = TASKS.write().unwrap();
                                        if let Some(temp) = tasks
                                            .iter_mut()
                                            .find(|x| x.id == complete_current_task.id)
                                        {
                                            temp.error_vec =
                                                serde_json::to_string_pretty(&error_vec).unwrap();
                                            temp.status = String::from("failed");
                                        }
                                    }
                                } else {
                                    on_event
                                        .send(DownloadEvent {
                                            id: complete_current_task.id,
                                            progress: progress_str.clone(),
                                            count: total,
                                            now_count: current_progress as i32,
                                            error_vec: serde_json::to_string_pretty(&error_vec)
                                                .unwrap(),
                                            status: String::from("finished"),
                                        })
                                        .unwrap();
                                    if let Err(e) = update_download_task_error_vec(
                                        complete_current_task.id,
                                        serde_json::to_string_pretty(&error_vec).unwrap().as_str(),
                                        "finished",
                                    ) {
                                        error!("save error_msg to db failed: {}", e);
                                    } else {
                                        let mut tasks = TASKS.write().unwrap();
                                        if let Some(temp) = tasks
                                            .iter_mut()
                                            .find(|x| x.id == complete_current_task.id)
                                        {
                                            temp.error_vec =
                                                serde_json::to_string_pretty(&error_vec).unwrap();
                                            temp.status = String::from("finished");
                                        }
                                    }
                                }
                            });
                    });
                } else if current_task_temp.dl_type == "current" {
                    let cache_json: Vec<Img> = serde_json::from_str(&cache_json_str).unwrap();
                    let semaphore = Arc::new(Semaphore::new(20));
                    let total = all_count;
                    let progress = Arc::new(Mutex::new(0));

                    thread::spawn(move || {
                        tokio::runtime::Builder::new_multi_thread()
                            .enable_all()
                            .build()
                            .unwrap()
                            .block_on(async {
                                let mut all_results = Vec::new();
                                let mut cache_json_sync = cache_json.clone();
                                let mut group_handles = Vec::<AbortHandle>::new();

                                let mut join_set = JoinSet::new();
                                let mut group_save_counter = 0;

                                for (i, url) in cache_json.into_iter().enumerate() {
                                    if url.done {
                                        let mut progress_lock = progress.lock().unwrap();
                                        *progress_lock += 1;
                                        continue;
                                    }
                                    let save_path_temp = comic_basic_path.join(format!(
                                        "{}/{}.jpg",
                                        complete_current_task.comic_name, i,
                                    ));
                                    let parent_path = save_path_temp.parent().unwrap();
                                    if !parent_path.exists() {
                                        fs::create_dir_all(parent_path).unwrap();
                                    }
                                    let save_path =
                                        save_path_temp.to_str().unwrap().to_string().clone();
                                    let url_str = url.href.clone();
                                    let semaphore = semaphore.clone();
                                    let progress = progress.clone();
                                    let handle = join_set.spawn(download_single_image(
                                        id, 0, i, url_str, save_path, semaphore, progress,
                                    ));
                                    group_handles.push(handle);
                                }

                                while let Some(res) = join_set.join_next().await {
                                    if let Ok(result) = res {
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

                                    group_save_counter += 1;
                                    if group_save_counter % 5 == 0 {
                                        // 保存进度到数据库
                                        let current_progress = *progress.lock().unwrap();

                                        info!("current current_progress: {}", current_progress);
                                        let progress_str = format!(
                                            "{:.2}",
                                            ((current_progress as f32) / (total as f32) * 100.00)
                                        );

                                        let event_res = on_event.send(DownloadEvent {
                                            id: complete_current_task.id,
                                            progress: progress_str.clone(),
                                            count: total,
                                            now_count: current_progress as i32,
                                            error_vec: String::from(""),
                                            status: String::from("downloading"),
                                        });
                                        match event_res {
                                            Ok(_s) => {}
                                            Err(e) => {
                                                error!("current on_event failed: {}", e);
                                            }
                                        }

                                        if let Err(e) = update_download_task_progress(
                                            complete_current_task.id,
                                            &progress_str,
                                            current_progress as i32,
                                            &serde_json::to_string_pretty(&cache_json_sync)
                                                .unwrap(),
                                        ) {
                                            error!("save current progress to db failed: {}", e);
                                        } else {
                                            let mut tasks = TASKS.write().unwrap();
                                            if let Some(temp) = tasks
                                                .iter_mut()
                                                .find(|x| x.id == complete_current_task.id)
                                            {
                                                temp.progress = progress_str;
                                                temp.now_count = current_progress as i32;
                                            }
                                        }
                                    }
                                }

                                // 确保最后一次进度也保存到数据库
                                let current_progress = *progress.lock().unwrap();
                                let progress_str = format!(
                                    "{:.2}",
                                    ((current_progress as f32) / (total as f32) * 100.00)
                                );

                                on_event
                                    .send(DownloadEvent {
                                        id: complete_current_task.id,
                                        progress: progress_str.clone(),
                                        count: total,
                                        now_count: current_progress as i32,
                                        error_vec: String::from(""),
                                        status: String::from("downloading"),
                                    })
                                    .unwrap();

                                if let Err(e) = update_download_task_progress(
                                    complete_current_task.id,
                                    &progress_str,
                                    current_progress as i32,
                                    &serde_json::to_string_pretty(&cache_json_sync).unwrap(),
                                ) {
                                    error!("save progress to db failed: {}", e);
                                } else {
                                    let mut tasks = TASKS.write().unwrap();
                                    if let Some(temp) =
                                        tasks.iter_mut().find(|x| x.id == complete_current_task.id)
                                    {
                                        temp.progress = progress_str.clone();
                                        temp.now_count = current_progress as i32;
                                    }
                                }

                                // 可以在这里进一步处理所有的下载结果 all_results
                                let mut error_vec: Vec<String> = Vec::new();
                                for result in all_results {
                                    if !result.error_msg.is_empty() {
                                        error_vec.push(result.error_msg);
                                    }
                                }
                                if !error_vec.is_empty() {
                                    error!("current Final result error: {:?}", &error_vec);

                                    on_event
                                        .send(DownloadEvent {
                                            id: complete_current_task.id,
                                            progress: progress_str.clone(),
                                            count: total,
                                            now_count: current_progress as i32,
                                            error_vec: serde_json::to_string_pretty(&error_vec)
                                                .unwrap(),
                                            status: String::from("failed"),
                                        })
                                        .unwrap();
                                    if let Err(e) = update_download_task_error_vec(
                                        complete_current_task.id,
                                        serde_json::to_string_pretty(&error_vec).unwrap().as_str(),
                                        "failed",
                                    ) {
                                        error!("save error_msg to db failed: {}", e);
                                    } else {
                                        let mut tasks = TASKS.write().unwrap();
                                        if let Some(temp) = tasks
                                            .iter_mut()
                                            .find(|x| x.id == complete_current_task.id)
                                        {
                                            temp.error_vec =
                                                serde_json::to_string_pretty(&error_vec).unwrap();
                                            temp.status = String::from("failed");
                                        }
                                    }
                                } else {
                                    on_event
                                        .send(DownloadEvent {
                                            id: complete_current_task.id,
                                            progress: progress_str.clone(),
                                            count: total,
                                            now_count: current_progress as i32,
                                            error_vec: serde_json::to_string_pretty(&error_vec)
                                                .unwrap(),
                                            status: String::from("finished"),
                                        })
                                        .unwrap();
                                    if let Err(e) = update_download_task_error_vec(
                                        complete_current_task.id,
                                        serde_json::to_string_pretty(&error_vec).unwrap().as_str(),
                                        "finished",
                                    ) {
                                        error!("save error_msg to db failed: {}", e);
                                    } else {
                                        let mut tasks = TASKS.write().unwrap();
                                        if let Some(temp) = tasks
                                            .iter_mut()
                                            .find(|x| x.id == complete_current_task.id)
                                        {
                                            temp.error_vec =
                                                serde_json::to_string_pretty(&error_vec).unwrap();
                                            temp.status = String::from("finished");
                                        }
                                    }
                                }
                            });
                    });
                }
            }
        } else {
            error!("update task status failed: {}, status: {}", id, &status);
            app.emit("info_msg_main", "update task status failed")
                .unwrap();
        }
    }
}

#[tauri::command]
fn get_tasks(_app: AppHandle) -> Vec<PartialDownloadTask> {
    info!("get_tasks");
    let tasks = TASKS.read().unwrap().clone();
    tasks
}
#[tauri::command]
fn delete_tasks(_app: AppHandle, id: i32) -> isize {
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
                                            &res.author,
                                            &current_name,
                                            "0.00",
                                            res.current_count as i32,
                                            0 as i32,
                                            "",
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
                                                    count: task.count,
                                                    now_count: task.now_count,
                                                    error_vec: task.error_vec,
                                                    done: task.done,
                                                };

                                                tasks.push(temp_task.clone());
                                                let tasks_to_log = (*tasks).clone();
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
    let mut no_find = true;
    match db_task_res {
        Ok(data) => {
            if data.is_empty() {
                no_find = true;
            } else {
                no_find = false;
                info!("already has this task!");
                app.emit_to("main", "info_msg_main", "already has this task!")
                    .unwrap();
                app.emit_to("main", "info_msg_add", "already has this task!")
                    .unwrap();
            }
        }
        Err(_e) => {
            no_find = true;
            error!("find_tasks_by_dl_type_and_url failed: {}", _e.to_string());
        }
    }
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
            &res.author,
            &res.comic_name,
            "0.00",
            all_count,
            0 as i32,
            "",
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
                    count: all_count,
                    now_count: task.now_count,
                    error_vec: task.error_vec,
                    done: task.done,
                };

                app.emit("new_task", &temp_task).unwrap();
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
                app.emit("err_msg_main", "insert juan task failed!")
                    .unwrap();
            }
        }
    }
}

// fn cal_task_progress(db_res: &Vec<PartialDownloadTask>) -> Vec<PartialDownloadTask> {
//     let cal_res = db_res.iter().map(|x| {
//         let temp = x.clone();
//     });
// }
