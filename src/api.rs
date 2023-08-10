use crate::CtFile;
use anyhow::{bail, Result};
use serde::Deserialize;
use std::str::FromStr;

const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:109.0) Gecko/20100101 Firefox/116.0";

macro_rules! url {
    ($url: expr; $( $k: expr => $v: expr ), *) => {{
        let mut url = reqwest::Url::parse($url)?;
        url.query_pairs_mut()$(.append_pair($k, &format!("{}", $v)))*;

        url
    }};
}

struct Link {
    /// File id defined by ctfile.
    file: String,
    /// Share password.
    password: Option<String>,
}

#[derive(Deserialize)]
pub struct CtFileObject {
    /// 文件名
    #[serde(rename(deserialize = "file_name"))]
    pub name: String,
    /// 文件大小，格式如 "627.20 MB"
    #[serde(rename(deserialize = "file_size"))]
    pub size: String,
    /// 发布时间，格式如 "2015-11-27"
    #[serde(rename(deserialize = "file_time"))]
    pub publish_date: String,
    #[serde(rename = "vip_dx_url")]
    /// VIP 链接
    vip_link: Option<String>,

    /// 上传者 ID
    #[serde(rename(deserialize = "userid"))]
    pub uploader: u64,
    /// 文件 ID
    #[serde(rename(deserialize = "file_id"))]
    pub unique_id: u64,
    /// 文件哈希值
    #[serde(rename(deserialize = "file_chk"))]
    pub checksum: String,
}

#[derive(Deserialize)]
pub struct CtFileSourceObject {
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

pub struct CtClient {
    http_client: reqwest::Client,
}

impl CtClient {
    pub fn new() -> Self {
        let http_client = reqwest::Client::new();
        Self { http_client }
    }

    async fn request(&self, url: reqwest::Url) -> Result<String> {
        let response = self
            .http_client
            .get(url)
            .header("User-Agent", DEFAULT_USER_AGENT)
            .send()
            .await?;

        let (status, body) = (response.status().as_u16(), response.text().await?);
        if status != 200 {
            bail!("Status code {}, server returns: {}", status, &body[..200]);
        }

        Ok(body)
    }

    async fn get_download_source(&self, uploader: u64, unique_id: u64, checksum: &str) -> Result<CtFileSourceObject> {
        let url = url!("https://webapi.ctfile.com/get_file_url.php";
            "uid" => uploader,
            "fid" => unique_id,
            "file_chk" => checksum,
            "app" => 0,
            "acheck" => 2,
            "rd" => random()
        );
        let body = self.request(url).await?;
        let result = serde_json::from_str::<CtFileSourceObject>(&body)?;
        Ok(result)
    }

    pub async fn get_file_by_id(&self, file_id: &str, password: &str, token: &str) -> Result<CtFile> {
        let count_separator = |file: &str| file.chars().filter(|ch| *ch == '-').count();
        let make_path = |file: &str| match count_separator(file) {
            1 => "file",
            _ => "f",
        };
        let url = url!("https://webapi.ctfile.com/getfile.php";
            "path" => make_path(file_id),
            "f" => file_id,
            "passcode" => password,
            "token" => token,
            "r" => random(),
            "ref" => "https://ctfile.qinlili.workers.dev"
        );
        let text = self.request(url).await?;

        if !text.starts_with("{\"code\":200,") {
            bail!("ctfile server returned {}", text);
        }

        #[derive(Deserialize)]
        struct Response {
            file: CtFileObject,
        }
        let Response { file } = serde_json::from_str::<Response>(&text)?;
        let CtFileObject {
            name,
            size,
            publish_date,
            checksum,
            uploader,
            unique_id,
            ..
        } = file;

        let source = self.get_download_source(uploader, unique_id, &checksum).await?;
        let CtFileSourceObject {
            url, name, exact_size, ..
        } = source;
        Ok(CtFile {
            name,
            publish_date,
            checksum,
            url,
            display_size: size,
            exact_size,
        })
    }

    pub async fn get_file_by_link(&self, link: &str, share_password: Option<String>, token: &str) -> Result<CtFile> {
        let Link {
            file,
            password: password_in_link,
        } = link.parse()?;
        let final_password = share_password // 先使用用户提供的密码
            .or(password_in_link) // 否则使用链接中的密码
            .unwrap_or_default(); // 否则使用空密码

        self.get_file_by_id(&file, &final_password, token).await
    }
}
