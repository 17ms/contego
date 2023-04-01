use contego::{common::Message, connector::Connector, listener::Listener};
use ntest::timeout;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    net::SocketAddr,
    path::PathBuf,
    thread,
};
use tokio::sync::mpsc;
use tokio_test::block_on;

#[test]
#[timeout(2000)]
/// Tests communication between GUI and individual handlers by mocking GUI signals.
fn filesync_signals() {
    let testdata = vec![
        ("1.txt", generate_data()),
        ("2.txt", generate_data()),
        ("3.txt", generate_data()),
    ];

    let mut paths = Vec::new();

    for file in &testdata {
        let filepath = String::from("./tests/data/") + file.0;
        let mut writer = BufWriter::new(File::create(filepath.clone()).unwrap());
        paths.push(filepath);
        writer.write_all(file.1.as_bytes()).unwrap();
    }

    let output_path = PathBuf::from("./tests/output/");
    let server_addr = SocketAddr::from(([127, 0, 0, 1], 9191));

    let (kill_server_tx, server_rx) = mpsc::channel::<Message>(2);
    let (server_tx, mut local_server_rx) = mpsc::channel::<Message>(2);
    let (local_client_tx, client_rx) = mpsc::channel::<Message>(2);
    let (client_tx, mut local_client_rx) = mpsc::channel::<Message>(2);

    let server_handle = thread::spawn(move || {
        let listener = Listener::new(server_addr, "xyz", 8192usize);
        block_on(listener.start(server_tx, server_rx, paths)).unwrap();
    });

    let server_channel_handle = thread::spawn(move || {
        block_on(local_server_rx.recv()).unwrap(); // ClientConnect
        block_on(local_server_rx.recv()).unwrap(); // ConnectionReady
        block_on(local_server_rx.recv()).unwrap(); // ClientDisconnect
        block_on(kill_server_tx.send(Message::Shutdown)).unwrap();
    });

    let client_handle = thread::spawn(move || {
        let output_path = output_path.clone();
        let connector = Connector::new(server_addr, "xyz", output_path);
        block_on(connector.connect(client_tx, client_rx)).unwrap()
    });

    let client_channel_handle = thread::spawn(move || {
        block_on(local_client_rx.recv()).unwrap(); // Metadata(HashMap)
        block_on(local_client_tx.send(Message::ClientReqAll)).unwrap();
        block_on(local_client_tx.send(Message::Shutdown)).unwrap();
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

fn generate_data() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect::<String>()
}
