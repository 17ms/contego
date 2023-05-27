use std::{collections::HashMap, error::Error, fs, net::SocketAddr, path::PathBuf};

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
    pub fn fetch(self, port: u16) -> Result<SocketAddr, Box<dyn Error + Send + Sync>> {
        debug!("Fetching IP information");

        let addr = match self {
            Ip::V4 => PUBLIC_IPV4,
            Ip::V6 => PUBLIC_IPV6,
            Ip::Local => {
                let addr_str = format!("127.0.0.1:{}", port);
                return Ok(addr_str.parse::<SocketAddr>()?);
            }
        };

        let res = format!("{}:{}", ureq::get(addr).call()?.into_string()?.trim(), port);
        let addr = res.parse::<SocketAddr>()?;

        debug!("IP: {}", res);

        Ok(addr)
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

fn filepaths(
    infile: Option<PathBuf>,
    files: Option<Vec<PathBuf>>,
) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    info!("Collecting filepaths");

    let mut filepaths = Vec::new();

    if let Some(infile) = infile {
        let paths = fs::read_to_string(infile)?;
        for path in paths.lines() {
            filepaths.push(PathBuf::from(path));
        }
    } else if let Some(files) = files {
        for file in files {
            filepaths.push(file);
        }
    }

    debug!("Filepaths collection finished (total: {})", filepaths.len());

    Ok(filepaths)
}

pub async fn metadata(
    files: &Vec<PathBuf>,
) -> Result<(Vec<FileInfo>, HashMap<String, PathBuf>), Box<dyn Error + Send + Sync>> {
    info!("Collecting metadata");

    let mut metadata = Vec::new();
    let mut index = HashMap::new();

    for path in files {
        let split = path.to_str().unwrap().split('/').collect::<Vec<&str>>();
        let name = split[split.len() - 1].to_string();
        let handle = File::open(path).await?;
        let size = handle.metadata().await?.len();
        let hash = crypto::try_hash(path)?;

        if size > 0 {
            debug!("Collecting '{}' metadata", name);

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
