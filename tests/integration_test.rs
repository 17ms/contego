use fragilebyte::{client, server};
use ntest::timeout;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use std::{
    fs::{read_to_string, remove_file, File},
    io::{BufWriter, Write},
    path::PathBuf,
    thread::{self, sleep},
    time::Duration,
};
use tokio_test::block_on;

#[test]
#[timeout(10000)]
/// Syncs three textfiles from ./data to ./output and checks
/// that their contents match.
fn sync_txt_files() {
    let data = vec![
        ("1.txt", create_data()),
        ("2.txt", create_data()),
        ("3.txt", create_data()),
    ];

    for file in &data {
        let filepath = String::from("./data/") + file.0;
        let mut writer = BufWriter::new(File::create(filepath).unwrap());
        writer.write_all(file.1.as_bytes()).unwrap();
    }

    let server_handle = thread::spawn(|| {
        block_on(server::listen(
            8080u16,
            PathBuf::from("./data"),
            8192usize,
            true,
            5,
        ))
        .unwrap();
    });

    // Sleep to give server time to start up
    sleep(Duration::from_millis(500));

    let client_handle = thread::spawn(|| {
        block_on(client::connect(
            String::from("127.0.0.1:8080"),
            PathBuf::from("./output"),
        ))
        .unwrap();
    });

    client_handle.join().unwrap();
    server_handle.join().unwrap();

    for file in data {
        let filepath = String::from("./output/") + file.0;
        let content = read_to_string(filepath).unwrap();

        assert_eq!(
            content, file.1,
            "Output [{}] does not match input [{}]",
            content, file.1
        );

        remove_file(String::from("./output/") + file.0).unwrap();
        remove_file(String::from("./data/") + file.0).unwrap();
    }
}

fn create_data() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect::<String>()
}
