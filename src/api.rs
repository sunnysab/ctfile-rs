use anyhow::Result;
use serde::Deserialize;
use std::str::FromStr;

const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64; rv:109.0) Gecko/20100101 Firefox/116.0";

struct Link {
    /// File id defined by ctfile.
    file: String,
    /// Share password.
    password: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CtFile {
    /// 文件名
    pub file_name: String,
    /// 文件大小，格式如 "627.20 MB"
    pub file_size: String,
    /// 发布时间，格式如 "2015-11-27"
    pub file_time: String,
    #[serde(rename = "vip_dx_url")]
    /// VIP 链接
    link: Option<String>,

    /// 上传者 ID
    pub userid: u64,
    /// 文件 ID
    pub file_id: u64,
    /// 文件哈希值
    pub file_chk: String,
}

#[derive(Debug, Deserialize)]
pub struct CtFileSource {
    code: u16,
    #[serde(rename = "downurl")]
    pub url: String,
    #[serde(rename = "file_name")]
    pub name: String,
    #[serde(rename = "file_size")]
    pub exact_size: usize,
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
    use rand::{thread_rng, Rng};

    let mut rng = thread_rng();
    rng.gen()
}

pub async fn get_file_by_id(file_id: &str, password: &str, token: &str) -> Result<CtFile> {
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

    #[derive(Debug, Deserialize)]
    struct Response {
        file: CtFile,
    }
    let response = serde_json::from_str::<Response>(&text)?;
    Ok(response.file)
}

pub async fn get_file_by_link(
    link: &str,
    share_password: Option<String>,
    token: &str,
) -> Result<CtFile> {
    let Link {
        file,
        password: password_in_link,
    } = link.parse()?;
    let final_password = share_password // 先使用用户提供的密码
        .or(password_in_link) // 否则使用链接中的密码
        .unwrap_or_default(); // 否则使用空密码

    get_file_by_id(&file, &final_password, token).await
}

impl CtFile {
    pub async fn get_download_source(&self) -> Result<CtFileSource> {
        const URL: &str = "https://webapi.ctfile.com/get_file_url.php";
        let mut url = reqwest::Url::parse(URL)?;

        url.query_pairs_mut()
            .append_pair("uid", &format!("{}", self.userid))
            .append_pair("fid", &format!("{}", self.file_id))
            .append_pair("file_chk", &self.file_chk)
            .append_pair("app", "0")
            .append_pair("acheck", "2")
            .append_pair("rd", &format!("{}", random()));
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

        let result = serde_json::from_str::<CtFileSource>(&text)?;
        Ok(result)
    }
}
