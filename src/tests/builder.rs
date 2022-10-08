use std::time::Duration;

use crate::{builder::*, msg::{MsgInternalBuilder, MsgBuilder, MsgSrv}};
use simplelog::{CombinedLogger, TermLogger, TerminalMode};
use tokio::{sync, task};

fn setup_log() {
    CombinedLogger::init(vec![
        TermLogger::new(
            log::LevelFilter::Debug,
            simplelog::Config::default(),
            TerminalMode::Mixed,
            simplelog::ColorChoice::Auto,
        ),
    ]);
}

macro_rules! broad_file_search_generate {
    ($files:expr) => {
        |_| $files.iter().map(|s| s.to_string()).collect()
    };
}

fn fs_read_file(s: String) -> anyhow::Result<String> {
    match s.as_str() {
        "./README.md" | ".\\README.md" => {
            Ok("# README".to_string())
        }
        "./test.md" | ".\\test.md" => {
            Ok("# test header".to_string())
        }
        _ => {
            assert!(false, "Should not be reached");
            Err(anyhow::anyhow!("Should not be reached"))
        }
    }
}

async fn fs_no_change(tx: sync::mpsc::Sender<MsgInternalBuilder>, _s: String) -> anyhow::Result<()> {
    tx.send(MsgInternalBuilder::Exit()).await.ok();
    Ok(())
}

#[tokio::test]
async fn test_initial_build() {
    setup_log();

    let (tx_file, rx_file) = sync::mpsc::channel(1);
    let (tx_srv, rx_srv) = sync::mpsc::channel(1);

    task::spawn(async move {
        builder_with_fs_change(
            tx_srv,
            ".".to_string(),
            rx_file,
            fs_no_change,
            fs_read_file,
            broad_file_search_generate!(vec!["README.md", "test.md"]),
        ).await;
    });

    tokio::time::sleep(Duration::from_secs(3)).await;

    let (tx_oneshot, rx_oneshot) = sync::oneshot::channel();
    assert!(tx_file.send(MsgBuilder::File("/README.md".to_string(), tx_oneshot)).await.is_ok());

    let (file, files) = rx_oneshot.await.expect("Expected builded file");
    assert_eq!(2, files.len(), "Expected 2 files");
    assert_eq!(Some(format!("<h1>README</h1>\n")), file);
}
