use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::PathBuf,
    str::FromStr,
};

use contego::{
    client::Client,
    server::Server,
    util::{metadata, Ip},
};
use env_logger::Env;
use log::debug;
use ntest::timeout;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use tokio::{fs::read_to_string, sync::mpsc};

#[tokio::test]
#[timeout(2000)]
/// Ensures backend communications integrity & the ability to handle individual requests.
async fn sockets_integration() {
    env_logger::Builder::from_env(Env::default().default_filter_or("debug"))
        .is_test(true)
        .try_init()
        .unwrap();
    //env_logger::builder().is_test(true).try_init().unwrap();

    debug!("Initializing and starting the test");

    let (testdata, paths) = testdata();
    let (metadata, index) = metadata(&paths).await.unwrap();

    let addr = Ip::Local.fetch(8080).unwrap();
    let outdir = PathBuf::from("./tests/output/");
    let key = String::from("testkey");
    let c_key = key.clone();

    let (tx, rx) = mpsc::channel::<()>(1);

    let server_handle = tokio::spawn(async move {
        debug!("Initializing the asynchronous server task");
        let server = Server::new(addr, key, 8192, metadata, index);
        debug!("Starting to listen to incoming connections");
        server.start(rx).await.unwrap();
    });

    let client_handle = tokio::spawn(async move {
        debug!("Initializing the asynchronous client task");
        let client = Client::new(addr, c_key, outdir);
        debug!("Connecting to the server");
        client.connection().await.unwrap();
    });

    client_handle.await.unwrap();
    tx.send(()).await.unwrap();
    server_handle.await.unwrap();

    debug!("Checking for file integrity");

    for file in testdata {
        let path = String::from("./tests/output/") + file.0;
        let recv_content = read_to_string(path).await.unwrap();

        assert_eq!(
            recv_content, file.1,
            "Output '{}' doesn't match the input '{}'",
            recv_content, file.1
        );

        fs::remove_file(String::from("./tests/output/") + file.0).unwrap();
        fs::remove_file(String::from("./tests/data/") + file.0).unwrap();

        debug!("File '{}' checked and removed successfully", file.0);
    }
}

fn testdata() -> (Vec<(&'static str, String)>, Vec<PathBuf>) {
    let mut paths = Vec::new();
    let testdata = vec![
        ("1.txt", generate_data()),
        ("2.txt", generate_data()),
        ("3.txt", generate_data()),
    ];

    for file in &testdata {
        let filepath = PathBuf::from_str("./tests/data/").unwrap().join(file.0);
        let mut writer = BufWriter::new(File::create(filepath.clone()).unwrap());
        paths.push(filepath);
        writer.write_all(file.1.as_bytes()).unwrap();
    }

    (testdata, paths)
}

fn generate_data() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect::<String>()
}
