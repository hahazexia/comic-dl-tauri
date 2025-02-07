use crate::models::*;
use crate::utils::ErrorMsg;
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use log::{error, info};
use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
// use rusqlite::{Connection, Result};

// pub static DB_CONNECTION: OnceLock<Mutex<Connection>> = OnceLock::new();
pub static DB_CONNECTION: OnceLock<Mutex<SqliteConnection>> = OnceLock::new();

pub fn init_db() -> Result<(), ErrorMsg> {
    if !db_file_exists() {
        create_db_file();
    }
    let db_path = get_db_path();

    // let conn_res = Connection::open(db_path);
    // match conn_res {
    //     Ok(conn) => {
    //         let _conn = DB_CONNECTION.set(Mutex::new(conn)).unwrap();
    //         Ok(())
    //     }
    //     Err(e) => {
    //         error!("db connection failed! {:?}", e);
    //         Err(ErrorMsg {
    //             msg: String::from("db connection failed!"),
    //         })
    //     }
    // }
    let conn_res = SqliteConnection::establish(&db_path);
    match conn_res {
        Ok(conn) => {
            let _conn = DB_CONNECTION.set(Mutex::new(conn));
            Ok(())
        }
        Err(e) => {
            error!("db connection failed! {:?}", e);
            Err(ErrorMsg {
                msg: String::from("db connection failed!"),
            })
        }
    }
}

pub fn create_table() -> QueryResult<()> {
    let mut conn = DB_CONNECTION.get().unwrap().lock().unwrap();
    let sql = r#"
        CREATE TABLE IF NOT EXISTS download_tasks (
            id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            dl_type TEXT NOT NULL,
            status TEXT NOT NULL,
            local_path TEXT NOT NULL,
            cache_json TEXT NOT NULL,
            url TEXT NOT NULL,
            author TEXT NOT NULL,
            comic_name TEXT NOT NULL,
            progress TEXT NOT NULL,
            count INTEGER NOT NULL,
            now_count INTEGER NOT NULL,
            error_vec TEXT NOT NULL,
            done BOOLEAN NOT NULL
        );
    "#;
    conn.batch_execute(sql)
}

// 插入新的下载任务
pub fn create_download_task(
    _dl_type: &str,
    _status: &str,
    _local_path: &str,
    _cache_json: &str,
    _url: &str,
    _author: &str,
    _comic_name: &str,
    _progress: &str,
    _count: i32,
    _now_count: i32,
    _error_vec: &str,
    _done: bool,
) -> QueryResult<DownloadTask> {
    use crate::schema::download_tasks::dsl::*;
    let mut conn = DB_CONNECTION.get().unwrap().lock().unwrap();

    let new_task = NewDownloadTask {
        dl_type: _dl_type,
        status: _status,
        local_path: _local_path,
        cache_json: _cache_json,
        url: _url,
        author: _author,
        comic_name: _comic_name,
        progress: _progress,
        count: _count,
        now_count: _now_count,
        error_vec: _error_vec,
        done: _done,
    };

    let row_id = diesel::insert_into(download_tasks)
        .values(&new_task)
        .returning(id)
        .get_result::<i32>(&mut *conn)?;
    info!("create_download_task row_id: {:?}", row_id);

    // 查询最新插入的记录
    download_tasks.find(row_id).first(&mut *conn)
}

// 更新下载任务
pub fn update_download_task_status(task_id: i32, status_temp: &str) -> QueryResult<usize> {
    use crate::schema::download_tasks::dsl::*;
    let mut conn = DB_CONNECTION.get().unwrap().lock().unwrap();

    diesel::update(download_tasks.find(task_id))
        .set((status.eq(status_temp)))
        .execute(&mut *conn)
}

pub fn update_download_task_progress(
    task_id: i32,
    _progress: &str,
    _now_count: i32,
    _cache_json: &str,
) -> QueryResult<usize> {
    use crate::schema::download_tasks::dsl::*;
    let mut conn = DB_CONNECTION.get().unwrap().lock().unwrap();

    diesel::update(download_tasks.find(task_id))
        .set((
            progress.eq(_progress),
            now_count.eq(_now_count),
            cache_json.eq(_cache_json),
        ))
        .execute(&mut *conn)
}
pub fn update_download_task_error_vec(
    task_id: i32,
    _error_vec: &str,
    _status: &str,
) -> QueryResult<usize> {
    use crate::schema::download_tasks::dsl::*;
    let mut conn = DB_CONNECTION.get().unwrap().lock().unwrap();

    diesel::update(download_tasks.find(task_id))
        .set((error_vec.eq(_error_vec), status.eq(_status)))
        .execute(&mut *conn)
}

// 删除下载任务
pub fn delete_download_task(task_id: i32) -> QueryResult<usize> {
    use crate::schema::download_tasks::dsl::*;
    let mut conn = DB_CONNECTION.get().unwrap().lock().unwrap();

    let row_id = diesel::delete(download_tasks.find(task_id))
        .returning(id)
        .get_result::<i32>(&mut *conn)?;

    info!("delete_download_task row_id: {}", row_id);

    Ok(row_id as usize)
}

// 查询下载任务
pub fn get_download_task(task_id: i32) -> QueryResult<DownloadTask> {
    use crate::schema::download_tasks::dsl::*;
    let mut conn = DB_CONNECTION.get().unwrap().lock().unwrap();

    download_tasks.find(task_id).first(&mut *conn)
}

// 返回 download_tasks 表中所有数据的函数 不包含字段 cache_json
pub fn get_all_download_tasks() -> QueryResult<Vec<PartialDownloadTask>> {
    use crate::schema::download_tasks::dsl::*;
    let mut conn = DB_CONNECTION.get().unwrap().lock().unwrap();

    download_tasks
        .select((
            id, dl_type, status, local_path, url, author, comic_name, progress, count, now_count,
            error_vec, done,
        ))
        .load::<PartialDownloadTask>(&mut *conn)
    // download_tasks.load::<DownloadTask>(&mut *conn)
}

// 根据 dl_type 和 url 查询下载任务
pub fn find_tasks_by_dl_type_and_url(
    target_dl_type: &str,
    target_url: &str,
) -> QueryResult<Vec<DownloadTask>> {
    use crate::schema::download_tasks::dsl::*;
    let mut conn = DB_CONNECTION.get().unwrap().lock().unwrap();

    download_tasks
        .filter(dl_type.eq(target_dl_type))
        .filter(url.eq(target_url))
        .load::<DownloadTask>(&mut *conn)
}

fn create_db_file() {
    let db_path = get_db_path();
    let db_dir = Path::new(&db_path).parent().unwrap();

    if !db_dir.exists() {
        fs::create_dir_all(db_dir).unwrap();
    }

    fs::File::create(db_path).unwrap();
}

fn db_file_exists() -> bool {
    let db_path = get_db_path();
    Path::new(&db_path).exists()
}

fn get_db_path() -> String {
    let home_dir = home::home_dir().unwrap();
    home_dir
        .join(".comic_dl_tauri/db/db.sqlite")
        .to_str()
        .unwrap()
        .to_string()
}
