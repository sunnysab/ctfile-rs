mod api;

use anyhow::Result;
use rand::Rng;
use serde::Deserialize;

const DEFAULT_TOKEN: &str = "5sijtqc2rlocvvkvmn7777";

struct Link {
    /// File id defined by ctfile.
    file: String,
    /// Share password.
    password: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CtFile {
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
struct CtFileSource {
    code: u16,
    #[serde(rename = "downurl")]
    url: String,
    #[serde(rename = "file_name")]
    name: String,
    #[serde(rename = "file_size")]
    exact_size: u32,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let file = api::get_file_by_id("4070316-134836896", "", DEFAULT_TOKEN).await?;
    let source = file.get_download_source().await?;
    println!("{file:#?}");
    println!("source = {source:#?}");
    Ok(())
}
