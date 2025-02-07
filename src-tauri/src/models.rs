use crate::schema::download_tasks;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Queryable, Selectable, PartialEq, Debug, Deserialize, Serialize, Clone)]
#[diesel(table_name = download_tasks)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DownloadTask {
    pub id: i32,
    pub dl_type: String,
    pub status: String,
    pub local_path: String,
    pub cache_json: String,
    pub url: String,
    pub author: String,
    pub comic_name: String,
    pub progress: String,
    pub count: i32,
    pub now_count: i32,
    pub error_vec: String,
    pub done: bool,
}

#[derive(Insertable, Queryable, Selectable, PartialEq, Debug, Deserialize, Serialize, Clone)]
#[diesel(table_name = download_tasks)]
pub struct NewDownloadTask<'a> {
    pub(crate) dl_type: &'a str,
    pub(crate) status: &'a str,
    pub(crate) local_path: &'a str,
    pub(crate) cache_json: &'a str,
    pub(crate) url: &'a str,
    pub(crate) author: &'a str,
    pub(crate) comic_name: &'a str,
    pub(crate) progress: &'a str,
    pub(crate) count: i32,
    pub(crate) now_count: i32,
    pub(crate) error_vec: &'a str,
    pub(crate) done: bool,
}

#[derive(Queryable, Selectable, PartialEq, Debug, Deserialize, Serialize, Clone)]
#[diesel(table_name = download_tasks)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct PartialDownloadTask {
    pub id: i32,
    pub dl_type: String,
    pub status: String,
    pub local_path: String,
    pub url: String,
    pub author: String,
    pub comic_name: String,
    pub progress: String,
    pub count: i32,
    pub now_count: i32,
    pub error_vec: String,
    pub done: bool,
}
