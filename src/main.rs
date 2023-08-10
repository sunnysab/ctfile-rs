pub mod api;
pub mod downloader;
mod protocol;

use anyhow::Result;
use bincode::{Decode, Encode};
use clap::Parser;
use prettytable::row;
use std::cell::RefCell;
use std::rc::Rc;
use tokio::net::TcpStream;

use tokio::task;

use crate::protocol::{BinStream, Command, CommandResult, TaskStatus};
pub use api::CtClient;
pub use downloader::DownloadQueue;

const DEFAULT_TOKEN: &str = "5sijtqc2rlocvvkvmn7777";
const DEFAULT_LISTEN_ADDR: &str = "localhost:7735";

#[derive(Debug, Encode, Decode)]
pub struct CtFile {
    /// 文件名
    pub name: String,
    /// 发布时间，格式如 "2015-11-27"
    pub publish_date: String,

    /// 文件哈希值
    pub checksum: String,
    /// 直链地址
    pub url: String,
    /// 文件大小，格式如 "627.20 MB"
    pub display_size: String,
    /// 资源字节数
    pub exact_size: usize,
}

#[derive(clap::Parser)]
#[command(name = "ctfile-rs")]
#[command(author = "sunnysab <i@sunnysab.cn>")]
#[command(version = "0.1")]
#[command(about = "Download file from ctfile.com via CLI.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    Daemon {
        #[arg(short, long)]
        listen: Option<String>,
    },
    Parse {
        url: String,
        #[arg(short, long)]
        password: Option<String>,
        #[arg(short, long)]
        token: Option<String>,
    },
    Download {
        url: String,
        #[arg(short, long)]
        password: Option<String>,
        #[arg(short, long)]
        token: Option<String>,
        #[arg(short, long)]
        daemon: Option<String>,
    },
    List {
        #[arg(short, long)]
        daemon: Option<String>,
    },
}

fn display_file_size(len: usize) -> String {
    let mut n: usize = 1024 * 1024 * 1024;
    let mut r = len / n;
    let t = ["GB", "MB", "KB", "Byte"];

    if len == 0 {
        return String::new();
    }
    let mut i: usize = 0;
    while r == 0 {
        n /= 1024;
        r = len / n;
        i += 1;
    }
    t[i].to_string()
}

async fn do_link_parsing(url: &str, password: Option<String>, token: Option<String>) -> Result<CtFile> {
    println!("URL {}", url);
    println!("Fetching file metadata...");
    let token = token.unwrap_or(DEFAULT_TOKEN.to_string());
    let client = CtClient::new();

    let file = client.get_file_by_link(url, password, &token).await?;
    println!(
        r#"File {} uploaded on {}\n
        Checksum {}\n
        Length: {} ({})\n
        Parsed result: {}"#,
        file.name,
        file.publish_date,
        file.checksum,
        file.exact_size,
        display_file_size(file.exact_size),
        file.url
    );
    Ok(file)
}

async fn daemon_task(queue: Rc<RefCell<DownloadQueue>>, socket: TcpStream) -> Result<()> {
    use protocol::Command;

    let mut stream = BinStream::new(socket);
    loop {
        match stream.recv::<Command>().await? {
            Command::List => {
                let cloned_task_queue: Vec<_> = queue.borrow().iter().map(TaskStatus::from).collect();
                let result = CommandResult::List(cloned_task_queue);

                stream.send(result).await?;
            }
            Command::Add(file) => {
                queue.borrow_mut().push(&file).await?;
            }
        }
    }
}

fn run_process_into_daemon() -> Result<()> {
    use daemonize::Daemonize;
    use std::fs::File;

    let stdout = File::create("/tmp/cffile-get-daemon.out")?;
    let stderr = File::create("/tmp/cffile-get-daemon.err")?;

    let daemonize = Daemonize::new()
        .pid_file("/tmp/ctfile-get.pid")
        .stdout(stdout)
        .stderr(stderr);

    daemonize.start().map_err(Into::into)
}

async fn daemon(queue: Rc<RefCell<DownloadQueue>>, listen: &str) -> Result<()> {
    run_process_into_daemon().expect("failed to run process into a daemon.");

    let socket = tokio::net::TcpListener::bind(listen).await?;
    while let Ok((socket, _)) = socket.accept().await {
        task::spawn_local(daemon_task(queue.clone(), socket));
    }
    Ok(())
}

fn print_list(status: &Vec<TaskStatus>) {
    use prettytable::row;
    use prettytable::{Table};

    let mut table = Table::new();
    table.add_row(row!["NAME", "RECEIVED", "TOTAL", "PROGRESS", "STATUS"]);
    for item in status {
        let name = &item.name[..20];
        let received = format!("{}", item.received);
        let total = format!("{}", item.total);
        let progress = format!("{:.0}%", item.received as f32 * 100.0f32 / item.total as f32);
        let status = if item.is_finished {
            String::from("FINISHED")
        } else {
            item.fail_message.clone().unwrap_or_else(|| "UNKNOWN".to_string())
        };

        table.add_row(row![name, received, total, progress, status]);
    }

    table.printstd();
}

async fn request_once<Req: Encode, Res: Decode>(target: &str, request: Req) -> Result<Res> {
    let socket = TcpStream::connect(target).await?;
    let mut stream = BinStream::new(socket);

    stream.send(request).await?;
    stream.recv::<Res>().await
}

async fn serve(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Daemon { listen } => {
            let addr = listen.unwrap_or(DEFAULT_LISTEN_ADDR.to_string());
            let queue = DownloadQueue::new();
            let queue = Rc::new(RefCell::new(queue));

            daemon(queue, &addr).await?;
        }
        Commands::Parse { url, password, token } => {
            let _ = do_link_parsing(&url, password, token).await?;
        }
        Commands::Download {
            url,
            password,
            token,
            daemon,
        } => {
            let file = do_link_parsing(&url, password, token).await?;
            if let Some(addr) = daemon {
                request_once(&addr, Command::Add(file)).await?;
            } else {
                use indicatif::ProgressBar;

                let task = downloader::download(&file, ".").await?;
                let pb = ProgressBar::new(file.exact_size as u64);

                while !task.progress.is_failed() && !task.progress.is_finished() {
                    let received = task.progress.received() as u64;
                    pb.set_position(received);
                }
                pb.finish();
            }
        }
        Commands::List { daemon } => {
            let addr = daemon.unwrap_or(DEFAULT_LISTEN_ADDR.to_string());
            let response: CommandResult = request_once(&addr, Command::List).await?;

            if let CommandResult::List(status) = response {
                print_list(&status);
            }
        }
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();

    let local = task::LocalSet::new();
    local
        .run_until(serve(cli))
        .await
        .expect("Unable to complete command...")
}
