use contego::{common::Message, connector::Connector, crypto, listener::Listener};
use ntest::timeout;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    net::SocketAddr,
    path::PathBuf,
    str::FromStr,
    thread,
};
use tokio::sync::mpsc;
use tokio_test::block_on;

#[test]
#[timeout(2000)]
/// Tests communication between UI and individual handlers by mocking signals.
fn filesync_signals() {
    let (testdata, paths) = write_testfiles();

    let output_path = PathBuf::from("./tests/output/");
    let addr = SocketAddr::from(([127, 0, 0, 1], 9191));
    let key = crypto::keygen();
    let c_key = key.clone();

    let (kstx, srx) = mpsc::channel::<Message>(10);
    let (stx, mut lsrx) = mpsc::channel::<Message>(10);
    let (lctx, crx) = mpsc::channel::<Message>(10);
    let (ctx, mut lcrx) = mpsc::channel::<Message>(10);

    let server_handle = thread::spawn(move || {
        let listener = Listener::new(addr, key, 8192usize).unwrap();
        block_on(listener.start(stx, srx, paths)).unwrap();
    });

    let server_channel_handle = thread::spawn(move || {
        block_on(lsrx.recv()).unwrap(); // ClientConnect
        block_on(lsrx.recv()).unwrap(); // ConnectionReady
        block_on(lsrx.recv()).unwrap(); // ClientDisconnect
        block_on(kstx.send(Message::Shutdown)).unwrap();
    });

    let client_handle = thread::spawn(move || {
        let output_path = output_path.clone();
        let connector = Connector::new(addr, c_key, output_path);
        block_on(connector.connect(ctx, crx)).unwrap()
    });

    let client_channel_handle = thread::spawn(move || {
        let metadata = block_on(lcrx.recv()).unwrap(); // Metadata(HashMap)

        if let Message::Metadata(inner) = metadata {
            assert_eq!(inner.len(), 3);
            for (filename, _) in inner {
                block_on(lctx.send(Message::ClientReq(filename))).unwrap();
            }
        }

        block_on(lctx.send(Message::Shutdown)).unwrap();
    });

    client_handle.join().unwrap();
    client_channel_handle.join().unwrap();
    server_handle.join().unwrap();
    server_channel_handle.join().unwrap();

    for file in testdata {
        fs::remove_file(String::from("./tests/output/") + file.0).unwrap();
        fs::remove_file(String::from("./tests/data/") + file.0).unwrap();
    }
}

fn write_testfiles() -> (Vec<(&'static str, String)>, Vec<PathBuf>) {
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
