use std::{
    io::{Error, ErrorKind::NotFound},
    net::{AddrParseError, SocketAddr},
    path::PathBuf,
};

pub fn addr_parser(addr: &str) -> Result<SocketAddr, AddrParseError> {
    let addr = addr
        .parse::<SocketAddr>()
        .expect("Failed to parse IP address");

    Ok(addr)
}

pub fn filepath_parser(path: &str) -> Result<PathBuf, Error> {
    let path = path.parse::<PathBuf>().expect("Failed to parse path");

    if path.exists() && path.is_file() {
        Ok(path)
    } else {
        Err(Error::new(NotFound, "File not found"))
    }
}

pub fn dirpath_parser(path: &str) -> Result<PathBuf, Error> {
    let path = path.parse::<PathBuf>().expect("Failed to parse path");

    if path.exists() && path.is_dir() {
        Ok(path)
    } else {
        Err(Error::new(NotFound, "Directory not found"))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn valid_ip() {
        use std::net::Ipv6Addr;

        let ipv4 = "10.1.2.3:8888";
        let ipv6 = "[2001:db8::1]:8888";

        let parsed_ipv4 = addr_parser(ipv4).unwrap();
        let parsed_ipv6 = addr_parser(ipv6).unwrap();

        assert_eq!(parsed_ipv4, SocketAddr::from(([10, 1, 2, 3], 8888)));
        assert_eq!(
            parsed_ipv6,
            SocketAddr::from((Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1), 8888))
        );
    }

    #[test]
    #[should_panic]
    fn short_ip() {
        let ip = "10.1.2:8888";
        addr_parser(ip).unwrap();
    }

    #[test]
    #[should_panic]
    fn long_ip() {
        let ip = "[2001:0db8:ac10:fe01:0000:0000:0000:0000:0000]:8888";
        addr_parser(ip).unwrap();
    }

    #[test]
    #[should_panic]
    fn ipv6_no_brackets() {
        let ip = "2001:db8::1:8888";
        addr_parser(ip).unwrap();
    }

    #[test]
    #[should_panic]
    fn ip_missing_port() {
        let ip = "10.1.2.3";
        addr_parser(ip).unwrap();
    }
}
