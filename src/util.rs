use std::{collections::HashMap, env, error::Error, fs, net::SocketAddr, path::PathBuf};

use log::{debug, info};
use tokio::{fs::File, io::BufWriter};

use crate::crypto;

const PUBLIC_IPV4: &str = "https://ipinfo.io/ip";
const PUBLIC_IPV6: &str = "https://ipv6.icanhazip.com";

#[derive(PartialEq, Eq)]
pub enum Ip {
    V4,
    V6,
    Local,
}

impl Ip {
    pub fn fetch(self, port: u16) -> Result<(SocketAddr, SocketAddr), Box<dyn Error>> {
        let addr = match self {
            Ip::V4 => PUBLIC_IPV4,
            Ip::V6 => PUBLIC_IPV6,
            Ip::Local => {
                let addr_str = format!("127.0.0.1:{}", port);
                let addr = addr_str.parse::<SocketAddr>()?;
                return Ok((addr, addr));
            }
        };

        info!("Fetching IP information from {}", addr);

        let res = format!("{}:{}", ureq::get(addr).call()?.into_string()?.trim(), port);
        let display_addr = res.parse::<SocketAddr>()?;
        let bind_addr = format!("0.0.0.0:{}", port).parse::<SocketAddr>()?;

        debug!("IP: {}", res);

        Ok((display_addr, bind_addr))
    }
}

#[derive(Clone)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub hash: String,
}

impl FileInfo {
    pub fn new(name: String, size: u64, hash: String) -> Self {
        Self { name, size, hash }
    }
}

pub fn filepaths(
    source: Option<PathBuf>,
    files: Option<Vec<PathBuf>>,
) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    info!("Collecting filepaths");

    let mut paths = Vec::new();

    if let Some(source) = source {
        let home = env::var("HOME")?;
        let content = fs::read_to_string(source)?;
        paths = content
            .lines()
            .into_iter()
            .map(|p| PathBuf::from(p.replace('~', &home)))
            .collect();
    } else if let Some(files) = files {
        paths = files;
    }

    debug!("Filepaths collection finished (total: {})", paths.len());

    Ok(paths)
}

pub async fn metadata(
    files: &Vec<PathBuf>,
) -> Result<(Vec<FileInfo>, HashMap<String, PathBuf>), Box<dyn Error>> {
    info!("Collecting metadata");

    let mut metadata = Vec::new();
    let mut index = HashMap::new();

    for path in files {
        debug!("Collecting '{}' metadata", path.to_str().unwrap());

        let split = path.to_str().unwrap().split('/').collect::<Vec<&str>>();
        let name = split[split.len() - 1].to_string();
        let handle = File::open(path).await?;
        let size = handle.metadata().await?.len();
        let hash = crypto::try_hash(path)?;

        if size > 0 {
            let info = FileInfo::new(name, size, hash.clone());
            metadata.push(info);
            index.insert(hash, path.clone());
        }
    }

    debug!(
        "Metadata collection successfully done (total: {})",
        metadata.len()
    );

    Ok((metadata, index))
}

pub async fn new_file(
    mut path: PathBuf,
    name: &str,
) -> Result<(BufWriter<File>, PathBuf), Box<dyn Error + Send + Sync>> {
    debug!("New file handle for '{}'", name);

    path.push(name);
    let handle = File::create(&path).await?;

    Ok((BufWriter::new(handle), path))
}

pub fn ascii() {
    let ascii = "                    __                 
  _________  ____  / /____  ____ _____ 
 / ___/ __ \\/ __ \\/ __/ _ \\/ __ `/ __ \\
/ /__/ /_/ / / / / /_/  __/ /_/ / /_/ /
\\___/\\____/_/ /_/\\__/\\___/\\__, /\\____/ 
                         /____/        ";
    println!("{}\n", ascii);
}
