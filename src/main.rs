use std::str::FromStr;

use anyhow::{Context, Result};
use rand::Rng;
use serde::Deserialize;

const DEFAULT_TOKEN: &str = "5sijtqc2rlocvvkvmn7777";

struct Link {
    /// File id defined by ctfile.
    file: String,
    /// Share password.
    password: Option<String>,
    /// Token, "5sijtqc2rlocvvkvmn7777" by default.
    token: Option<String>,
}

impl FromStr for Link {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        todo!()
    }
}

fn random() -> f64 {
    let mut rng = rand::thread_rng();
    rng.gen()
}


#[derive(Debug, Deserialize)]
struct FileBasicInfo {
    /// 文件名
    file_name: String,
    /// 文件大小，格式如 "627.20 MB"
    file_size: String,
    /// 发布时间，格式如 "2015-11-27"
    file_time: String,
    #[serde(rename = "vip_dx_url")]
    /// VIP 链接
    link: Option<String>,

    /// 上传者 ID
    userid: u64,
    /// 文件 ID
    file_id: u64,
    /// 文件哈希值
    file_chk: String,
}

#[derive(Debug, Deserialize)]
struct Response {
    file: FileBasicInfo,
}

async fn get_url_by_id(file_id: &str, password: &str, token: &str) -> Result<FileBasicInfo> {
    const URL: &str = "https://webapi.ctfile.com/getfile.php";

    let count_separator = |file: &str| file.chars().filter(|ch| *ch == '-').count();
    let make_path = |file: &str| if count_separator(file) == 1 { "file" } else { "f" };

    let mut url = reqwest::Url::parse(URL)?;
    url.query_pairs_mut().append_pair("path", make_path(file_id)).append_pair("f", file_id).append_pair("passcode", password).append_pair("token", token).append_pair("r", &format!("{}", random())).append_pair("ref", "https://ctfile.qinlili.workers.dev");

    let client = reqwest::Client::new();
    let response = client.get(url).header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64; rv:109.0) Gecko/20100101 Firefox/116.0").send().await?;
    let text = response.text().await?;

    if !response.status().is_success() {
        anyhow::bail!("Status code {}, server returns: {}", response.status().as_u16(), text);
    }
    if !text.starts_with("{\"code\":200,") {
        anyhow::bail!("Interface returns {}", text);
    }

    let response = response.json::<Response>().await?;
    Ok(response.file)
}

async fn get_download_url_by_link(link: &str, password: &str, token: &str) -> Result<FileBasicInfo> {
    let url = reqwest::Url::parse(link)
        .context(|| format!("failed to parse link {link}"))?;

    let path = url.path();
    let (_, file_id) = path.rsplit_once("/");
    let password = if let Some((k, v)) = url.query_pairs().filter(|(k, v)| k == 'p').next() {
        v.as_ref()
    } else {
        password
    };
    get_url_by_id(file_id, &password, token).await
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let _ = get_url_by_id("4070316-134836896", "", DEFAULT_TOKEN).await?;

    Ok(())
}
