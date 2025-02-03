use crate::utils::{
    cache_html, extract_number_manual, get_url_query, read_file_to_string, read_from_json,
    retry_request, save_to_json, StatusCode,
};
use futures::future::join_all;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::AppHandle;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[allow(dead_code)]
pub struct HandleHtmlRes {
    code: StatusCode,
    data: DataWrapper,
    msg: String,
    comic_name: String,
    current_name: String,
    current_count: u32,
    done: bool,
}

#[allow(dead_code)]
impl HandleHtmlRes {
    pub fn code(&self) -> StatusCode {
        self.code
    }
    pub fn data(&self) -> DataWrapper {
        self.data.clone()
    }
    pub fn new() -> HandleHtmlRes {
        HandleHtmlRes {
            code: StatusCode::Failed,
            data: DataWrapper::HashMapData(HashMap::new()),
            msg: String::from(""),
            comic_name: String::from(""),
            current_name: String::from(""),
            current_count: 0,
            done: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum DataWrapper {
    HashMapData(HashMap<String, Vec<CurrentElement>>),
    VecData(Vec<String>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[allow(dead_code)]
pub struct CurrentElement {
    name: String,
    href: String,
    imgs: Vec<String>,
    count: u32,
    done: bool,
}

#[allow(dead_code)]
pub struct JuanHuaFanwaiCount {
    juan: usize,
    hua: usize,
    fanwai: usize,
    all: usize,
}

pub async fn handle_html(url: String, dl_type: String, _app: &AppHandle) -> HandleHtmlRes {
    info!("handle_html invoke url: {}, dl_type: {}", &url, &dl_type);

    match dl_type.clone().as_str() {
        "juan" | "hua" | "fanwai" | "juan_hua_fanwai" => {
            let res = handle_comic_html(url.clone()).await;
            res
        }
        "current" => {
            let res = handle_current_html(url.clone()).await;
            res
        }
        "author" => HandleHtmlRes {
            code: StatusCode::Success,
            msg: String::from(""),
            data: DataWrapper::HashMapData(HashMap::new()),
            comic_name: String::from(""),
            current_name: String::from(""),
            current_count: 0,
            done: true,
        },
        _ => HandleHtmlRes {
            code: StatusCode::Failed,
            msg: String::from("no matched dl_type"),
            data: DataWrapper::HashMapData(HashMap::new()),
            comic_name: String::from(""),
            current_name: String::from(""),
            current_count: 0,
            done: false,
        },
    }
}

pub async fn handle_comic_html(url: String) -> HandleHtmlRes {
    // 获取漫画页面 kuid
    let kuid = get_url_query(url.clone(), String::from("kuid"));
    // 系统的用户目录
    let home_dir = home::home_dir().unwrap();
    let comic_html_cache_path = home_dir.join(format!(
        ".comit_dl_tauri/html_cache/antbyw_comic_{}.htmlcache",
        &kuid
    ));
    let comic_json_cache_path = home_dir.join(format!(
        ".comit_dl_tauri/json_cache/antbyw_comic_{}.json",
        &kuid
    ));

    let mut json_data_from_read = Some(HandleHtmlRes::new());

    // 如果已经存在current cache json 直接返回
    if comic_json_cache_path.exists() {
        match read_from_json::<HandleHtmlRes>(comic_json_cache_path.to_str().unwrap()) {
            Ok(res) => {
                json_data_from_read = Some(res.clone());
                if res.done {
                    return res;
                } else {
                    warn!("comic cache not done!");
                    if let DataWrapper::HashMapData(data) = res.data.clone() {
                        for (key, value) in data.iter() {
                            let all = value.len();
                            let mut current: usize = 0;
                            for i in value.iter() {
                                if i.done {
                                    current += 1;
                                }
                            }
                            warn!("comic cache {}: {}/{}", key, current, all);
                        }
                    }
                }
            }
            Err(_e) => {
                warn!(
                    "read comic cache json failed! comic_json_cache_path: {}",
                    comic_json_cache_path.to_str().unwrap()
                );
            }
        };
    }

    // 先读缓存，如果没有再去下载漫画页html然后缓存到本地
    let html_content;
    if comic_html_cache_path.exists() {
        html_content = read_file_to_string(comic_html_cache_path.to_str().unwrap()).unwrap();
    } else {
        // 请求漫画页面html
        match retry_request(&url, 5).await {
            Ok(s) => {
                html_content = s;
            }
            Err(_) => {
                return HandleHtmlRes {
                    code: StatusCode::Failed,
                    msg: String::from("download comic html failed!"),
                    data: DataWrapper::HashMapData(HashMap::new()),
                    comic_name: String::from(""),
                    current_name: String::from(""),
                    current_count: 0,
                    done: false,
                };
            }
        };

        // 缓存漫画页面html
        if let Err(e) = cache_html(&html_content, comic_html_cache_path) {
            error!(
                "cache {} failed: {}",
                format!("antbyw_{}.htmlcache", &kuid),
                e
            );
        }
    }

    let mut title_vec = Vec::new();
    let mut content_vec = Vec::new();
    let mut json_data: HashMap<String, Vec<CurrentElement>> = HashMap::new();
    let mut comic_name: String = String::from("");

    if let Some(data) = json_data_from_read {
        json_data = if let DataWrapper::HashMapData(temp_data) = data.data {
            temp_data
        } else {
            HashMap::new()
        };
        comic_name = data.comic_name;
    }

    if json_data.is_empty() {
        let document = scraper::Html::parse_document(&html_content);

        // 获取漫画名
        let name_selector = scraper::Selector::parse(".uk-heading-line.mt10.m10.mbn").unwrap();
        comic_name = document
            .select(&name_selector)
            .next()
            .unwrap()
            .to_owned()
            .inner_html();

        // 获取三个 title 单行本 单话 番外篇
        let juan_hua_fanwai_title_selector =
            &scraper::Selector::parse("h3.uk-alert-warning").unwrap();
        let juan_hua_fanwai_title = document.select(juan_hua_fanwai_title_selector).to_owned();
        for ele in juan_hua_fanwai_title {
            let temp = ele.inner_html();
            title_vec.push(temp);
        }

        // 获取三个 title 下面的所有链接
        let juan_hua_fanwai_content_selector =
            &scraper::Selector::parse(".uk-container .uk-switcher.uk-margin").unwrap();
        let juan_hua_fanwai_content = document.select(juan_hua_fanwai_content_selector).to_owned();

        // 循环三个类别，处理下面的所有链接
        for ele in juan_hua_fanwai_content {
            let a_selector = &scraper::Selector::parse("a.zj-container").unwrap();
            let a_elements = ele.select(a_selector).to_owned();
            // href_vec 数组存储一个类别下面所有的页面链接
            let mut href_vec = Vec::new();
            for a_ele in a_elements {
                let name = a_ele.inner_html();
                let mut href = a_ele.value().attr("href").unwrap().to_owned().to_string();
                href.remove(0);
                let complete_url = format!("https://www.antbyw.com{}", href);
                let temp = CurrentElement {
                    name: name,
                    href: complete_url,
                    imgs: Vec::new(),
                    count: 0,
                    done: false,
                };
                href_vec.push(temp);
            }
            href_vec.sort_by(|a, b| {
                let a_number = extract_number_manual(&a.name).unwrap();
                let b_number = extract_number_manual(&b.name).unwrap();
                a_number.cmp(&b_number)
            });
            content_vec.push(href_vec);
        }

        // json_data最终处理好的HashMap结构
        // {
        //     "单话": [{name: "第1话", href: ""}],
        //     "单行本": [{name: "第1卷", href: ""}],
        //     "番外篇": [{name: "番外1", href: ""}],
        // }
        for (title, content) in title_vec.iter().zip(content_vec.iter()) {
            json_data.insert(title.clone(), content.clone());
        }
    }

    // 并发获取comic（juan hua fanwai）中所有的 current页的图片
    let mut new_json_data: HashMap<String, Vec<CurrentElement>> = HashMap::new();
    let juan_count = json_data.get("单行本").unwrap().len();
    let hua_count = json_data.get("单话").unwrap().len();
    let fanwai_count = json_data.get("番外篇").unwrap().len();

    let all_type_count = JuanHuaFanwaiCount {
        juan: juan_count,
        hua: hua_count,
        fanwai: fanwai_count,
        all: juan_count + hua_count + fanwai_count,
    };

    for (key, value) in json_data.iter() {
        let task_count: usize = value.len();
        const GROUP_SIZE: usize = 5;

        let mut concurrent_results: Vec<HandleHtmlRes> = Vec::new();

        let all_tasks: Vec<String> = (1..=task_count)
            .map(|index| value.get(index - 1).unwrap().href.clone())
            .collect();
        for group in all_tasks.chunks(GROUP_SIZE) {
            let group_tasks = group
                .iter()
                .map(|current_url| handle_current_html(current_url.to_string().clone()));
            let results: Vec<HandleHtmlRes> = join_all(group_tasks).await;
            concurrent_results.extend(results);
        }

        let mut new_value: Vec<CurrentElement> = Vec::new();

        for (i, data) in value.iter().enumerate() {
            let mut temp = data.clone();
            let res = concurrent_results.get(i).unwrap();
            if res.code == StatusCode::Success {
                if let DataWrapper::VecData(res_temp) = res.data.clone() {
                    temp.imgs = res_temp;
                }
                temp.count = concurrent_results.get(i).unwrap().current_count;
                temp.done = true;
            } else {
                temp.imgs = Vec::new();
                temp.count = 0;
                temp.done = false;
            }
            new_value.push(temp);
        }

        new_json_data.insert(key.to_string(), new_value);
    }

    let done: bool;
    let mut temp_count: usize = 0;
    for (_key, value) in new_json_data.iter() {
        for data in value.iter() {
            if data.done {
                temp_count += 1;
            }
        }
    }
    if temp_count == all_type_count.all {
        done = true;
    } else {
        done = false;
    }

    let res = HandleHtmlRes {
        code: StatusCode::Success,
        msg: String::from(""),
        data: DataWrapper::HashMapData(new_json_data),
        comic_name: comic_name.clone(),
        current_name: String::from(""),
        current_count: 0,
        done: done,
    };
    // 缓存漫画页json数据
    if let Err(e) = save_to_json(&res, &comic_json_cache_path.to_str().unwrap()) {
        error!(
            "cache comic json {}: {} ",
            format!("antbyw_{}.json", &kuid),
            e,
        );
        return HandleHtmlRes {
            code: StatusCode::Failed,
            msg: String::from("cache comic json failed!"),
            data: DataWrapper::HashMapData(HashMap::new()),
            comic_name: comic_name,
            current_name: String::from(""),
            current_count: 0,
            done: false,
        };
    }
    res
}

pub async fn handle_current_html(url: String) -> HandleHtmlRes {
    // https://www.antbyw.com/plugin.php?id=jameson_manhua&a=read&kuid=169197&zjid=1218556

    // 获取current页面zjid
    let zjid = get_url_query(url.clone(), String::from("zjid"));

    // 系统的用户目录
    let home_dir = home::home_dir().unwrap();
    let current_html_cache_path = home_dir.join(format!(
        ".comit_dl_tauri/html_cache/antbyw_current_{}.htmlcache",
        &zjid
    ));
    let current_json_cache_path = home_dir.join(format!(
        ".comit_dl_tauri/json_cache/antbyw_current_{}.json",
        &zjid
    ));

    // 如果已经存在current cache json 直接返回
    if current_json_cache_path.exists() {
        match read_from_json::<HandleHtmlRes>(current_json_cache_path.to_str().unwrap()) {
            Ok(res) => {
                if res.done {
                    return res;
                } else {
                    if let DataWrapper::VecData(imgs) = res.data.clone() {
                        warn!(
                            "current cache json imgs.len: {}, current_count: {}",
                            imgs.len(),
                            res.current_count
                        );
                    }
                }
            }
            Err(_e) => {
                warn!(
                    "read current cache json failed! current_json_cache_path: {}",
                    current_json_cache_path.to_str().unwrap()
                );
            }
        };
    }

    // 先读缓存，如果没有再去下载current页html然后缓存到本地
    let html_content;
    if current_html_cache_path.exists() {
        html_content = read_file_to_string(current_html_cache_path.to_str().unwrap()).unwrap();
    } else {
        // 请求current页面html
        match retry_request(&url, 5).await {
            Ok(s) => {
                html_content = s;
            }
            Err(_) => {
                return HandleHtmlRes {
                    code: StatusCode::Failed,
                    msg: String::from("download current html failed!"),
                    data: DataWrapper::HashMapData(HashMap::new()),
                    comic_name: String::from(""),
                    current_name: String::from(""),
                    current_count: 0,
                    done: false,
                };
            }
        };

        // 缓存current页面html
        if let Err(e) = cache_html(&html_content, current_html_cache_path) {
            error!(
                "cache {} failed: {}",
                format!("antbyw_current_{}.htmlcache", &zjid),
                e
            );
        }
    }

    let comic_name;
    let current_name;
    let mut href_vec = Vec::new();
    let current_count;

    {
        let document_current = scraper::Html::parse_document(&html_content);

        // 获取漫画名
        let name_selector = scraper::Selector::parse(".uk-breadcrumb.pl0 a").unwrap();
        let breads: Vec<_> = document_current.select(&name_selector).to_owned().collect();
        comic_name = breads.last().unwrap().to_owned().inner_html().to_owned();
        // 获取current名
        let name_selector = scraper::Selector::parse(".uk-breadcrumb.pl0 span").unwrap();
        let breads_span: Vec<_> = document_current.select(&name_selector).to_owned().collect();
        current_name = breads_span
            .last()
            .unwrap()
            .to_owned()
            .inner_html()
            .to_owned();
        // 获取图片数量
        let count_selector = scraper::Selector::parse(".uk-badge.ml8").unwrap();
        let count_span = document_current
            .select(&count_selector)
            .next()
            .to_owned()
            .unwrap();
        current_count = extract_number_manual(&count_span.inner_html().to_owned()).unwrap();

        // 获取current所有图片href
        let img_selector = scraper::Selector::parse(".uk-zjimg img").unwrap();
        let imgs: Vec<_> = document_current.select(&img_selector).to_owned().collect();

        for ele in imgs {
            let href = ele.attr("data-src").unwrap().to_owned();
            href_vec.push(href.to_string());
        }
    }

    let done = if href_vec.len() == current_count as usize {
        true
    } else {
        false
    };
    let res = HandleHtmlRes {
        code: StatusCode::Success,
        msg: String::from(""),
        data: DataWrapper::VecData(href_vec.clone()),
        comic_name: comic_name.clone(),
        current_name: current_name.clone(),
        current_count: current_count.clone(),
        done: done,
    };

    // 缓存current页json数据
    if let Err(e) = save_to_json(&res, &current_json_cache_path.to_str().unwrap()) {
        error!(
            "cache current json {}: {} ",
            format!("antbyw_current_{}.json", &zjid),
            e,
        );
        return HandleHtmlRes {
            code: StatusCode::Failed,
            msg: String::from("cache current json failed!"),
            data: DataWrapper::VecData(Vec::new()),
            comic_name: comic_name,
            current_name: current_name,
            current_count: current_count,
            done: false,
        };
    }

    res
}
