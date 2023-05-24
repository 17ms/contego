use std::{error::Error, fs, net::SocketAddr, path::PathBuf};

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
        let addr = match self {
            Ip::V4 => PUBLIC_IPV4,
            Ip::V6 => PUBLIC_IPV6,
            Local => {
                let addr_str = format!("127.0.0.1:{}", port);
                return addr_str.parse::<SocketAddr>();
            }
        };

        let res = format!("{}:{}", ureq::get(addr).call()?.into_string()?.trim(), port);
        let addr = res.parse::<SocketAddr>()?;

        Ok(addr)
    }
}

fn filepaths(
    infile: Option<PathBuf>,
    files: Option<Vec<PathBuf>>,
) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut filepaths = Vec::new();

    if let Some(infile) = infile {
        let paths = fs::read_to_string(infile)?;
        for path in paths.lines() {
            filepaths.push(PathBuf::from(path));
        }
    }

    if let Some(files) = files {
        for file in files {
            filepaths.push(file);
        }
    }

    Ok(filepaths)
}
