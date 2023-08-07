use std::str::FromStr;

use anyhow::Result;
use rand::Rng;
use serde::Deserialize;

const DEFAULT_TOKEN: &str = "5sijtqc2rlocvvkvmn7777";

const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64; rv:109.0) Gecko/20100101 Firefox/116.0";

struct Link {
    /// File id defined by ctfile.
    file: String,
    /// Share password.
    password: Option<String>,
}

impl FromStr for Link {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        // Typical url: https://306t.com/file/4070316-134836896?p=[password]
        let url = reqwest::Url::parse(s)?;
        let path = url.path(); // path = "/file/4070316-134836896"
        let (_, file_id) = path.rsplit_once("/").unwrap(); // file id = "4070316-134836896"
        let password = if let Some((_, v)) = url.query_pairs().filter(|(k, v)| k == "p").next() {
            Some(v.to_string())
        } else {
            None
        };

        Ok(Link {
            file: String::from(file_id),
            password,
        })
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
    let make_path = |file: &str| match count_separator(file) {
        1 => "file",
        _ => "f",
    };

    let mut url = reqwest::Url::parse(URL)?;
    url.query_pairs_mut()
        .append_pair("path", make_path(file_id))
        .append_pair("f", file_id)
        .append_pair("passcode", password)
        .append_pair("token", token)
        .append_pair("r", &format!("{}", random()))
        .append_pair("ref", "https://ctfile.qinlili.workers.dev");

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header("User-Agent", DEFAULT_USER_AGENT)
        .send()
        .await?;
    let (status, text) = (response.status().as_u16(), response.text().await?);

    if status != 200 {
        anyhow::bail!("Status code {}, server returns: {}", status, text);
    }
    if !text.starts_with("{\"code\":200,") {
        anyhow::bail!("Interface returns {}", text);
    }

    let response = serde_json::from_str::<Response>(&text)?;
    Ok(response.file)
}

async fn get_download_url_by_link(
    link: &str,
    share_password: Option<String>,
    token: &str,
) -> Result<FileBasicInfo> {
    let Link {
        file,
        password: password_in_link,
    } = link.parse()?;
    let final_password = share_password // 先使用用户提供的密码
        .or(password_in_link) // 否则使用链接中的密码
        .unwrap_or_default(); // 否则使用空密码

    get_url_by_id(&file, &final_password, token).await
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let result = get_url_by_id("4070316-134836896", "", DEFAULT_TOKEN).await?;

    println!("{result:#?}");
    Ok(())
}
