#[allow(dead_code)]
use image::ImageFormat;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::{self, File};
use std::io::Error;
use std::io::{self, Read, Write};
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use url::Url;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
#[allow(dead_code)]
pub enum StatusCode {
    Success,
    Failed,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct ErrorMsg {
    msg: String,
}

#[warn(dead_code)]
impl ErrorMsg {
    pub fn new(msg: &str) -> ErrorMsg {
        ErrorMsg {
            msg: msg.to_string(),
        }
    }

    pub fn to_string(&self) -> String {
        self.msg.clone()
    }
}

impl fmt::Display for ErrorMsg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl std::error::Error for ErrorMsg {}

// 根据image库类型返回图片格式字符串
pub fn format_to_string(format: &ImageFormat) -> &'static str {
    match format {
        ImageFormat::Jpeg => "JPEG",
        ImageFormat::Png => "PNG",
        ImageFormat::Gif => "GIF",
        ImageFormat::Bmp => "BMP",
        ImageFormat::Tiff => "TIFF",
        ImageFormat::WebP => "WEBP",
        ImageFormat::Pnm => "PNM",
        ImageFormat::Tga => "TGA",
        ImageFormat::Dds => "DDS",
        ImageFormat::Ico => "ICO",
        ImageFormat::Hdr => "HDR",
        ImageFormat::OpenExr => "OPENEXR",
        ImageFormat::Farbfeld => "FARBFELD",
        ImageFormat::Avif => "AVIF",
        ImageFormat::Qoi => "QOI",
        ImageFormat::Pcx => "PCX",
        _ => "other unknown format",
    }
}

// 处理图片url
pub fn handle_url(url_string: String) -> String {
    let mut res: String = "".to_string();
    if let Ok(url) = Url::parse(&url_string) {
        if let Some(d) = url.domain() {
            // println!("Domain: {}", d);
            let d_vec = split_string(d, ".");
            let last_two = &d_vec[d_vec.len() - 2..];
            let mut new_array: Vec<&str> = last_two.iter().map(|s| s.as_str()).collect();
            new_array.insert(0, "www");

            res = format!("https://{}", join_strings(new_array, "."));
        } else {
            println!("Domain not found in the URL");
        }
    } else {
        println!("Invalid URL format");
    }
    res
}

// 字符串转数组
pub fn split_string(input: &str, delimiter: &str) -> Vec<String> {
    input.split(delimiter).map(|s| s.to_string()).collect()
}

// 数组元素连接成字符串
pub fn join_strings(strings: Vec<&str>, delimiter: &str) -> String {
    strings.join(delimiter)
}

// 从图片url中获取图片扩展名
pub fn handle_img_extension(url_string: String) -> String {
    let mut res = "";
    if let Some(index) = url_string.rfind('.') {
        res = &url_string[index + 1..];
        // println!("File extension: {}", res);
    } else {
        println!("File extension not found in the URL");
    }
    res.to_string()
}

// 基于手动遍历提取数字
pub fn extract_number_manual(input: &str) -> Option<u32> {
    let mut result = String::new();
    let mut in_number = false;

    for c in input.chars() {
        if c.is_digit(10) {
            result.push(c);
            in_number = true;
        } else if in_number {
            break;
        }
    }

    if result.is_empty() {
        Some(0)
    } else {
        match result.parse::<u32>() {
            Ok(number) => Some(number),
            Err(_) => Some(0),
        }
    }
}

// 获取本地目录的名称
#[allow(dead_code)]
pub fn get_dir_name<P: AsRef<Path>>(path: P) -> Option<String> {
    let path = path.as_ref();
    path.file_name()
        .and_then(|name| name.to_str().map(|s| s.to_string()))
}

// 判断一个路径是否是图片文件
#[allow(dead_code)]
pub fn is_image_file(path: &Path) -> bool {
    match path.extension().and_then(|s| s.to_str()) {
        Some(ext) => matches!(
            ext.to_lowercase().as_str(),
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tiff" | "webp"
        ),
        None => false,
    }
}

// 获取不带扩展名的文件名
#[allow(dead_code)]
pub fn get_file_name_without_extension(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

// 获取二级域名
pub fn get_second_level_domain(url_str: &str) -> Option<String> {
    let url = Url::parse(url_str).ok()?;

    let host = url.host_str()?;

    let parts: Vec<&str> = host.split('.').collect();

    if parts.len() >= 2 {
        Some(parts[parts.len() - 2].to_string())
    } else {
        None
    }
}

pub fn create_file_if_not_exists(file_name: &str) -> std::io::Result<()> {
    // 检查文件是否存在
    if !Path::new(&file_name).exists() {
        // 如果文件不存在，创建目录
        fs::create_dir_all(Path::new(&file_name).parent().unwrap())?;

        // 创建文件
        fs::File::create(&file_name)?;
        // println!("File created: {}", file_name);
    } else {
        // println!("File already exists: {}", file_name);
    }

    Ok(())
}

pub fn read_file_to_string(file_path: &str) -> Result<String, Error> {
    // 读取文件内容并返回字符串
    let content = fs::read_to_string(file_path)?;
    Ok(content)
}

pub fn write_string_to_file(file_path: &PathBuf, content: &str) -> Result<(), Error> {
    fs::write(file_path, content)?;
    Ok(())
}

pub fn get_url_query(url_str: String, key: String) -> String {
    let pairs = Url::parse(&url_str).unwrap();
    for (key_temp, value) in pairs.query_pairs() {
        let temp = key_temp.as_ref();
        if temp == &key {
            return value.as_ref().to_string();
        }
    }
    String::from("")
}

pub fn create_cache_dir() -> Result<StatusCode, ErrorMsg> {
    info!("create_cache_dir invoke");
    let home_dir = home::home_dir();

    if let Some(home_dir_temp) = home_dir {
        let final_path = home_dir_temp.join(".comit_dl_tauri");
        let html_cache_path = final_path.join("html_cache");
        let json_cache_path = final_path.join("json_cache");
        let log_path = final_path.join("log");

        if !final_path.exists() {
            if let Err(e) = fs::create_dir_all(&final_path) {
                error!("create_cache_dir final_path error: {e}");
                return Err(ErrorMsg::new(
                    format!(
                        "create {} directory failed!",
                        &final_path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or(".comit_dl_tauri")
                    )
                    .as_str(),
                ));
            }
        }
        if !html_cache_path.exists() {
            if let Err(e) = fs::create_dir_all(&html_cache_path) {
                error!("create_cache_dir html_cache_path error: {e}");
                return Err(ErrorMsg::new(
                    format!(
                        "create {} directory failed!",
                        &html_cache_path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("html_cache")
                    )
                    .as_str(),
                ));
            }
        }
        if !json_cache_path.exists() {
            if let Err(e) = fs::create_dir_all(&json_cache_path) {
                error!("create_cache_dir json_cache_path error: {e}");
                return Err(ErrorMsg::new(
                    format!(
                        "create {} directory failed!",
                        &json_cache_path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("json_cache")
                    )
                    .as_str(),
                ));
            }
        }
        if !log_path.exists() {
            if let Err(e) = fs::create_dir_all(&log_path) {
                error!("create_cache_dir log_path error: {e}");
                return Err(ErrorMsg::new(
                    format!(
                        "create {} directory failed!",
                        &log_path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("log")
                    )
                    .as_str(),
                ));
            }
        }

        return Ok(StatusCode::Success);
    } else {
        return Err(ErrorMsg::new("get home directory failed!"));
    }
}

pub fn cache_html(html: &str, name: PathBuf) -> Result<StatusCode, ErrorMsg> {
    if let Err(e) = write_string_to_file(&name, html) {
        return Err(ErrorMsg::new(&format!(
            "cache html failed: {} {}",
            name.to_str().unwrap(),
            e,
        )));
    }
    Ok(StatusCode::Success)
}

// 将数据保存到 JSON 文件
pub fn save_to_json<T>(data: &T, file_path: &str) -> io::Result<()>
where
    T: Serialize,
{
    // 将数据序列化为 JSON 字符串
    let json_str = serde_json::to_string_pretty(data)?;
    // 打开文件以写入模式
    let mut file = File::create(file_path)?;
    // 将 JSON 字符串写入文件
    file.write_all(json_str.as_bytes())?;
    Ok(())
}

// 从 JSON 文件中读取数据
pub fn read_from_json<T>(file_path: &str) -> io::Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    // 打开文件以读取模式
    let mut file = File::open(file_path)?;
    // 创建一个可变的 String 来存储文件内容
    let mut json_str = String::new();
    // 从文件中读取内容到 json_str
    file.read_to_string(&mut json_str)?;
    // 将 JSON 字符串反序列化为 T 类型的数据
    let data: T = serde_json::from_str(&json_str)?;
    Ok(data)
}

// 下载html重试函数
pub async fn retry_request(url: &str, max_retries: u32) -> Result<String, reqwest::Error> {
    let mut retries = 0;

    loop {
        // 尝试创建客户端
        let client_result = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build();

        let client = match client_result {
            Ok(client) => client,
            Err(err) => {
                retries += 1;
                if retries >= max_retries {
                    return Err(err);
                }
                error!(
                    "Client creation failed (attempt {}): {:?}. Retrying... url: {}",
                    retries, err, &url
                );
                continue;
            }
        };

        retries = 0;

        // 发送请求
        match client.get(url).send().await {
            Ok(response) => {
                let html_content = response.text().await?;
                return Ok(html_content);
            }
            Err(err) => {
                retries += 1;
                if retries >= max_retries {
                    return Err(err);
                }
                error!(
                    "Request failed (attempt {}): {:?}. Retrying... url: {}",
                    retries, err, &url
                );
            }
        }
    }
}
