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

async fn fs_change_add_test(tx: sync::mpsc::Sender<MsgInternalBuilder>, _s: String) -> anyhow::Result<()> {
    tx.send(MsgInternalBuilder::FileCreated("test.md".to_string())).await.unwrap();
    tokio::time::sleep(Duration::from_secs(5)).await;
    tx.send(MsgInternalBuilder::Exit()).await.ok();
    Ok(())
}

#[tokio::test]
async fn test_initial_build() {
    setup_log();

    let (tx_file, rx_file) = sync::mpsc::channel(1);
    let (tx_srv, mut rx_srv) = sync::mpsc::channel(1);

    let builder_handle = {
        let tx_file = tx_file.clone();

        task::spawn(async move {
            builder_with_fs_change(
                tx_srv,
                ".".to_string(),
                tx_file,
                rx_file,
                fs_no_change,
                fs_read_file,
                broad_file_search_generate!(vec!["README.md", "test.md"]),
            ).await;
        })
    };

    let (tx_oneshot, rx_oneshot) = sync::oneshot::channel();
    let (tx_oneshot_test, rx_oneshot_test) = sync::oneshot::channel();
    assert!(tx_file.send(MsgBuilder::File("/README.md".to_string(), tx_oneshot)).await.is_ok());
    assert!(tx_file.send(MsgBuilder::File("/test.md".to_string(), tx_oneshot_test)).await.is_ok());

    let (file, files) = rx_oneshot.await.expect("Expected builded file");
    let (file_test, _) = rx_oneshot_test.await.expect("Expected builded file");

    assert!(builder_handle.await.is_ok());

    assert_eq!(2, files.len(), "Expected 2 files");
    assert_eq!(Some(format!("<h1>README</h1>\n")), file);
    assert_eq!(Some(format!("<h1>test header</h1>\n")), file_test);
}

#[tokio::test]
async fn test_add_file() {
    setup_log();

    let (tx_file, rx_file) = sync::mpsc::channel(1);
    let (tx_srv, mut rx_srv) = sync::mpsc::channel(1);

    let builder_handle = {
        let tx_file = tx_file.clone();

        task::spawn(async move {
        builder_with_fs_change(
                tx_srv,
                ".".to_string(),
                tx_file.clone(),
                rx_file,
                fs_change_add_test,
                fs_read_file,
                broad_file_search_generate!(vec!["README.md"]),
            ).await;
        })
    };

    let (tx_oneshot, rx_oneshot) = sync::oneshot::channel();
    assert!(tx_file.send(MsgBuilder::File("/README.md".to_string(), tx_oneshot)).await.is_ok());
    let (file, _) = rx_oneshot.await.expect("Expected builded file");

    log::debug!("{:?}", rx_srv.recv().await);

    let (tx_oneshot_test, rx_oneshot_test) = sync::oneshot::channel();
    assert!(tx_file.send(MsgBuilder::File("/test.md".to_string(), tx_oneshot_test)).await.is_ok());
    let (file_test, files) = rx_oneshot_test.await.expect("Expected builded file");

    log::debug!("{:?}", rx_srv.recv().await);
    assert!(builder_handle.await.is_ok());

    assert_eq!(2, files.len(), "Expected 2 files");
    assert_eq!(Some(format!("<h1>README</h1>\n")), file);
    assert_eq!(Some(format!("<h1>test header</h1>\n")), file_test);
}
