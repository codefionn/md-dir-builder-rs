/*
 *  md-dir-builder serve markdown files in a given directory
 *  Copyright (C) 2022 Fionn Langhans
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *  GNU General Public License for more details.
 *
 *  You should have received a copy of the GNU General Public License
 */
use crate::markdown::MarkdownParser;
use crate::msg::MsgBuilder;
use crate::msg::MsgInternalBuilder;
use ahash::RandomState;
use serde::Deserialize;
use std::{cmp::Ordering, collections::HashMap, path::Path, sync::Arc};
use tokio::{
    runtime::Builder,
    sync::{self, Mutex},
    task,
};

#[cfg(watchman)]
use watchman_client::prelude::*;

use anyhow::anyhow;

use super::MsgSrv;
use std::fs;

static IGNORE_DIRS: &[&'static str] = &[".git"];

fn broad_dir_search(path_str: &String) -> Box<Vec<String>> {
    let path_str_clone = std::rc::Rc::new(path_str.clone());
    let path = Path::new(&path_str);
    if let Ok(files) = path.read_dir() {
        files
            .map(|entry| match entry {
                Ok(entry) => {
                    let file_name = std::rc::Rc::new(entry.file_name().into_string().unwrap());
                    if entry.path().is_dir() {
                        if IGNORE_DIRS.contains(&file_name.as_str()) {
                            Box::new(vec![])
                        } else {
                            let file_name_iter = [file_name.to_string()];
                            let newdir = format!("{}/{}", &path_str_clone, file_name);
                            Box::new(
                                broad_dir_search(&newdir)
                                    .iter()
                                    .map(|path| format!("{}/{}", file_name, path))
                                    .chain(file_name_iter)
                                    .collect(),
                            )
                        }
                    } else {
                        Box::new(vec![])
                    }
                }
                Err(err) => {
                    log::error!(
                        "Error in directory {} reading an entry: {}",
                        path_str_clone,
                        err
                    );

                    Box::new(vec![])
                }
            })
            .fold(Box::new(vec![]), |mut x, mut y| {
                x.append(&mut y);
                x
            })
    } else {
        log::error!("Could not read directory {}", path_str);

        Box::new(vec![])
    }
}

fn broad_file_search(path_str: String) -> Vec<String> {
    let path_str_clone = std::rc::Rc::new(path_str.clone());
    let path = Path::new(&path_str);
    if let Ok(files) = path.read_dir() {
        files
            .map(|entry| match entry {
                Ok(entry) => {
                    let file_name = std::rc::Rc::new(entry.file_name().into_string().unwrap());
                    if entry.path().is_dir() {
                        if IGNORE_DIRS.contains(&file_name.as_str()) {
                            vec![]
                        } else {
                            let newdir = format!("{}/{}", &path_str_clone, file_name);
                            broad_file_search(newdir)
                                .iter()
                                .map(|path| format!("{}/{}", file_name, path))
                                .collect()
                        }
                    } else {
                        if file_name.ends_with(".md") {
                            vec![format!("{}", file_name)]
                        } else {
                            vec![]
                        }
                    }
                }
                Err(err) => {
                    log::error!(
                        "Error in directory {} reading an entry: {}",
                        path_str_clone,
                        err
                    );
                    vec![]
                }
            })
            .flatten()
            .collect()
    } else {
        log::error!("Could not read directory {}", path_str);

        vec![]
    }
}

/// Process a markdown file. Converts it to HTML and saves the result in ``map``.
/// During processing, the a lock is generated in ``processing``.
///
/// ## Result
///
/// Returns ``true`` on success, otherwise ``false``.
async fn process_file(
    dir: &Path,
    file_str: &String,
    map: Arc<Mutex<HashMap<String, String, RandomState>>>,
    files: Arc<Mutex<Vec<String>>>,
    processing: Arc<Mutex<HashMap<String, Arc<Mutex<()>>, RandomState>>>,
) -> bool {
    let webpath = format!("/{}", file_str);
    log::debug!("Processing file {}", webpath);

    processing
        .lock()
        .await
        .insert(webpath.clone(), Arc::new(Mutex::new(())));

    let mut success = true;

    let path = dir.join(file_str);
    match fs::read_to_string(path.clone()) {
        Ok(result) => {
            let html = crate::markdown::CommonMarkParser::default().parse_to_html(result.as_str());
            map.lock().await.insert(webpath.clone(), html);
            let mut files = files.lock().await;
            if !files.contains(&webpath) {
                files.push(webpath.clone());
            }
        }
        Err(err) => {
            log::error!(
                "Error occured reading file {}: {}",
                path.to_str().unwrap(),
                err
            );
            success = false;
        }
    }

    processing.lock().await.remove(&webpath);

    log::debug!("Processed file {}", webpath);

    return success;
}

async fn sort_files(files: Arc<Mutex<Vec<String>>>) {
    files.lock().await.sort_by(|a, b| -> Ordering {
        let cnt_dir_a = a.matches("/").count();
        let cnt_dir_b = b.matches("/").count();

        if cnt_dir_a != cnt_dir_b {
            cnt_dir_b.cmp(&cnt_dir_a)
        } else {
            a.cmp(b)
        }
    });
}

/// Creats the Markdown to HTML builder
///
/// This watches the given directory ``path_str`` and rebuilds new or modified files.
pub async fn builder(
    tx_srv: sync::mpsc::Sender<MsgSrv>,
    path_str: String,
    mut rx_file: sync::mpsc::Receiver<MsgBuilder>,
) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let path = Path::new(&path_str);

    if !path.exists() {
        log::error!("Directory {} does not exist", &path_str);
        tx_srv.send(MsgSrv::Exit()).await.unwrap();
        return;
    }

    if !path.is_dir() {
        log::error!("Path {} is not a directory", &path_str);
        tx_srv.send(MsgSrv::Exit()).await.unwrap();
        return;
    }

    let map: Arc<Mutex<HashMap<String, String, RandomState>>> = Arc::new(Mutex::new(
        HashMap::with_capacity_and_hasher(128, RandomState::new()),
    ));

    let files: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::with_capacity(128)));

    let processing: Arc<Mutex<HashMap<String, Arc<Mutex<()>>, RandomState>>> = Arc::new(
        Mutex::new(HashMap::with_capacity_and_hasher(128, RandomState::new())),
    );

    let files_to_build = {
        let path_str = path_str.clone();
        broad_file_search(path_str)
    };

    log::debug!("Starting file builder");

    // Listen to queries from the server
    {
        let map = map.clone();
        let processing = processing.clone();
        let files = files.clone();

        rt.spawn(async move {
            server_queries(rx_file, processing, map, files).await;
        });
    }

    // Listen to file (created,modified,deleted) events and react accordingly
    let (tx_builder, mut rx_builder) = sync::mpsc::channel(128);
    {
        let path_str = path_str.clone();
        let map = map.clone();
        let files = files.clone();
        let processing = processing.clone();

        rt.spawn(async move {
            file_builder(rx_builder, tx_srv, path_str, processing, map, files).await;
        });
    }

    // Initial build step: Builds all detected files
    {
        let path_str = path_str.clone();
        let map = map.clone();
        let processing = processing.clone();
        let files_clone = files.clone();
        let files = files.clone();

        rt.spawn(async move {
            initial_build(files_to_build, path_str, processing, map, files).await;
        });
    }

    #[cfg(watchman)]
    if let Err(err) = watcher_watchman(tx_builder, &path, &path_str).await {
        log::error!("An error occured watching files: {}", err);
    }

    #[cfg(notify)]
    #[cfg(not(watchman))]
    if let Err(err) = watcher_notify(tx_builder, &path, &path_str).await {
        log::error!("An error occured watching files: {}", err);
    }

    // Listen to file changes in the specified directory
    #[cfg(not(notify))]
    #[cfg(not(watchman))]
    if let Err(err) = watch_inotify(tx_builder.clone(), &path, &path_str).await {
        log::error!("An error occured watching files: {}", err);
    }

    log::debug!("Exited builder files");
}

async fn server_queries(
    mut rx_file: sync::mpsc::Receiver<MsgBuilder>,
    processing: Arc<Mutex<HashMap<String, Arc<Mutex<()>>, RandomState>>>,
    map: Arc<Mutex<HashMap<String, String, RandomState>>>,
    files: Arc<Mutex<Vec<String>>>,
) {
    log::debug!("Started file reader");

    while let Some(msg) = rx_file.recv().await {
        log::debug!("File reader event: {:?}", msg);

        match msg {
            MsgBuilder::File(path, result) => {
                if let Some(lock) = processing.lock().await.get(&path) {
                    let _ = lock.lock().await; // Wait for processing to finish
                }

                let files = files.lock().await.clone().into_iter().collect();

                if let Some(content) = map.lock().await.get(&path) {
                    result
                        .send((Some(content.clone()), files))
                        .unwrap_or_else(|err| log::error!("{:?}", err));
                } else {
                    result
                        .send((None, files))
                        .unwrap_or_else(|err| log::error!("{:?}", err));
                }
            }
            MsgBuilder::AllFiles(result) => {
                let files = files.lock().await.clone().into_iter().collect();
                result
                    .send(files)
                    .unwrap_or_else(|err| log::error!("{:?}", err));
            }
            MsgBuilder::Exit() => {
                break;
            }
        }
    }

    log::debug!("Exited file communication");
}

async fn file_builder(
    mut rx_builder: sync::mpsc::Receiver<MsgInternalBuilder>,
    tx_srv: sync::mpsc::Sender<MsgSrv>,
    path_str: String,
    processing: Arc<Mutex<HashMap<String, Arc<Mutex<()>>, RandomState>>>,
    map: Arc<Mutex<HashMap<String, String, RandomState>>>,
    files: Arc<Mutex<Vec<String>>>,
) {
    log::debug!("Started file builder listener");
    let path = Path::new(&path_str);

    while let Some(msg) = rx_builder.recv().await {
        log::debug!("File builder event: {:?}", msg);

        match msg {
            MsgInternalBuilder::FileCreated(file) => {
                if process_file(&path, &file, map.clone(), files.clone(), processing.clone()).await
                {
                    let webpath = format!("/{}", file);
                    log::debug!(
                        "Sending processed file {} to server (is_new: {})",
                        webpath,
                        true
                    );
                    let content = map.lock().await.get(&webpath).unwrap().clone();
                    sort_files(files.clone()).await;
                    tx_srv
                        .send(MsgSrv::NewFile(webpath, files.lock().await.clone()))
                        .await
                        .unwrap();
                }
            }
            MsgInternalBuilder::FileModified(file) => {
                if process_file(&path, &file, map.clone(), files.clone(), processing.clone()).await
                {
                    let webpath = format!("/{}", file);
                    log::debug!("Sending processed file {} to server", webpath);
                    let content = map.lock().await.get(&webpath).unwrap().clone();
                    tx_srv.send(MsgSrv::File(webpath, content)).await.unwrap();
                }
            }
            MsgInternalBuilder::FileDeleted(file) => {}
            MsgInternalBuilder::Exit() => {
                break;
            }
        }
    }

    log::debug!("Exited file builder listener");
}

async fn initial_build(
    files_to_build: Vec<String>,
    path_str: String,
    processing: Arc<Mutex<HashMap<String, Arc<Mutex<()>>, RandomState>>>,
    map: Arc<Mutex<HashMap<String, String, RandomState>>>,
    files: Arc<Mutex<Vec<String>>>,
) {
    for file in files_to_build {
        let map = map.clone();
        let processing = processing.clone();
        let files = files.clone();
        let path = Path::new(&path_str);
        process_file(path, &file, map, files, processing).await;
    }

    sort_files(files).await;
}

#[cfg(not(notify))]
#[cfg(not(watcherman))]
async fn watch_inotify(
    tx_builder: sync::mpsc::Sender<MsgInternalBuilder>,
    path: &Path,
    path_str: &String,
) -> anyhow::Result<()> {
    use inotify::{EventMask, Inotify, WatchMask};

    let mut inotify = Inotify::init()?;

    let mut wd_to_dir = HashMap::new();

    wd_to_dir.insert(
        inotify.add_watch(
            path,
            WatchMask::MODIFY | WatchMask::CREATE | WatchMask::DELETE,
        )?,
        String::new(),
    );

    let dirs = broad_dir_search(path_str);
    for dir in dirs.iter() {
        let dir_path = path.join(dir);
        wd_to_dir.insert(
            inotify.add_watch(
                dir_path,
                WatchMask::MODIFY | WatchMask::CREATE | WatchMask::DELETE,
            )?,
            dir.to_string(),
        );
    }

    log::debug!("Watching current directory for activity...");

    let mut buffer = [0u8; 4096];
    loop {
        let events = inotify.read_events_blocking(&mut buffer)?;

        let is_markdown = regex::Regex::new(r"\.md$").unwrap();

        for event in events {
            if let (Some(dir), Some(name)) = (wd_to_dir.get(&event.wd), event.name) {
                let file = if dir.is_empty() {
                    name.to_string_lossy().to_string()
                } else {
                    format!("{}/{}", dir, name.to_string_lossy())
                };

                if event.mask.contains(EventMask::CREATE) {
                    if event.mask.contains(EventMask::ISDIR) {
                        log::debug!("Directory created: {}/{:?}", dir, event.name);
                    } else if is_markdown.is_match(file.as_str()) {
                        log::debug!("File created: {}/{:?}", dir, event.name);
                        tx_builder
                            .send(MsgInternalBuilder::FileCreated(file))
                            .await
                            .unwrap();
                    }
                } else if event.mask.contains(EventMask::DELETE) {
                    if event.mask.contains(EventMask::ISDIR) {
                        log::debug!("Directory deleted: {}/{:?}", dir, event.name);
                    } else if is_markdown.is_match(file.as_str()) {
                        log::debug!("File deleted: {}", file);
                        tx_builder
                            .send(MsgInternalBuilder::FileDeleted(file))
                            .await
                            .unwrap();
                    }
                } else if event.mask.contains(EventMask::MODIFY) {
                    if event.mask.contains(EventMask::ISDIR) {
                        log::debug!("Directory modified: {}/{:?}", dir, event.name);
                    } else if is_markdown.is_match(file.as_str()) {
                        log::debug!("File modified: {}", file);
                        tx_builder
                            .send(MsgInternalBuilder::FileModified(file))
                            .await
                            .unwrap();
                    }
                }
            }
        }
    }
}

#[cfg(notify)]
#[cfg(not(watchman))]
async fn watcher_notify(
    tx: sync::mpsc::Sender<MsgInternalBuilder>,
    path: &Path,
    path_str: &String,
) -> anyhow::Result<()> {
    let real_path = match path.canonicalize() {
        Ok(real_path) => real_path.to_string_lossy().to_string(),
        _ => path_str.clone(),
    };
    log::debug!("Started watcher notify in {}", real_path);

    use notify::*;

    let (tx, mut rx) = sync::mpsc::channel(128);

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            tx.send(res);
        },
        Config::default(),
    )?;

    watcher.watch(path.as_ref(), RecursiveMode::Recursive)?;

    log::debug!("Started watching paths with notify");

    while let Some(res) = rx.recv().await {
        log::debug!("FS-Event: {:?}", res);
    }

    log::debug!("Exited watcher notify");

    Ok(())
}

#[cfg(watchman)]
query_result_type! {
    struct WatchResult { name: NameField
    }
}

#[cfg(watchman)]
async fn watcher_watchman(
    tx: sync::mpsc::Sender<MsgInternalBuilder>,
    path: &Path,
    path_str: &String,
) -> anyhow::Result<()> {
    let client = Connector::new().connect().await?;
    let path = CanonicalPath::canonicalize(&path)?;
    let resolved_root = client.resolve_root(path).await?;
    let match_expr = Expr::Match(MatchTerm {
        glob: "*.md".to_string(),
        wholename: false,
        include_dot_files: false,
        no_escape: true,
    });
    let (mut subscription, response) = client
        .subscribe::<WatchResult>(
            &resolved_root,
            SubscribeRequest {
                since: None,
                relative_root: None,
                expression: Some(match_expr),
                fields: vec!["name"],
                empty_on_fresh_instance: true,
                case_sensitive: true,
                defer_vcs: false,
                defer: vec![],
                drop: vec![],
            },
        )
        .await?;

    log::debug!(
        "Started watch files at {} with watchman v{}",
        path_str,
        response.version
    );

    while let Ok(data) = subscription.next().await {
        log::debug!("{:?}", data);

        use watchman_client::SubscriptionData;
        match data {
            SubscriptionData::FilesChanged(result) => {
                log::debug!("Files changed: {:?}", result.files);
            }
            _ => {}
        }
    }

    Ok(())
}
