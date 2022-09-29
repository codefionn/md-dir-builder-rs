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
mod builder;
mod markdown;
mod msg;
mod router;
mod ui;

use log::LevelFilter;
use msg::{MsgBuilder, MsgSrv};
use simplelog::{CombinedLogger, TermLogger, TerminalMode, WriteLogger};

use std::{
    net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6},
    path::Path,
    process::exit,
};

use clap::Parser;
use tokio::{sync, task};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, clap::ValueEnum)]
pub enum ParserType {
    CommonMark,
    Pandoc,
}

/// Program to create webserver for markdown files
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Port to host service at
    #[clap(short, long, value_parser, default_value_t = 8080)]
    port: u16,

    /// Directory where to serve the markdown files from
    #[clap(short, long, value_parser, default_value = ".")]
    directory: String,

    /// Parser used to transform markdown files to HTML
    #[clap(long, value_enum, default_value_t = ParserType::CommonMark)]
    parser: ParserType,

    /// Be verbose
    #[clap(short, long, value_parser, default_value_t = false)]
    verbose: bool,

    /// Open in browser
    #[clap(long = "no-open", value_parser, default_value_t = false)]
    no_open: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let level_filter = if args.verbose {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    CombinedLogger::init(vec![
        TermLogger::new(
            level_filter,
            simplelog::Config::default(),
            TerminalMode::Mixed,
            simplelog::ColorChoice::Auto,
        ),
        //WriteLogger::new(LevelFilter::Info, Config::default(), File::create("my_rust_binary.log").unwrap()),
    ])
    .expect("Failed initializing logger");

    let (tx_srv, mut rx_srv) = sync::mpsc::channel(256);
    let (tx_file, rx_file) = sync::mpsc::channel(256);

    let ((app4, tx4), (app6, tx6)) = tokio::join!(
        router::create_router(tx_file.clone()),
        router::create_router(tx_file.clone())
    );

    let addr4 = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), args.port).into();
    let addr6 = SocketAddrV6::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1), args.port, 0, 0).into();

    let server4 = axum::Server::try_bind(&addr4);
    let server6 = axum::Server::try_bind(&addr6);

    let server4 = server4
        .unwrap_or_else(|_| panic!("Port {} is already in use", args.port))
        .serve(app4.into_make_service());
    let server6 = server6
        .unwrap_or_else(|_| panic!("Port {} is already in use", args.port))
        .serve(app6.into_make_service());

    task::spawn(async move {
        let (server4, server6) = tokio::join!(server4, server6);

        server4.unwrap();
        server6.unwrap();
    });

    log::info!(
        "Started servers on http://[::1]:{} and http://127.0.0.1:{}",
        args.port,
        args.port
    );

    task::spawn(async move {
        builder::builder(tx_srv, args.directory, rx_file).await;
    });

    if !args.no_open {
        webbrowser::open(format!("http://[::1]:{}", args.port).as_str()).ok();
    }

    log::debug!("Server is now ready");

    while let Some(msg) = rx_srv.recv().await {
        log::debug!("General server event: {:?}", msg);

        match msg {
            MsgSrv::File(path, content) => {
                let (msg0, msg1) = tokio::join!(
                    tx4.send(MsgSrv::File(path.clone(), content.clone())),
                    tx6.send(MsgSrv::File(path, content))
                );

                msg0.unwrap();
                msg1.unwrap();
            }
            MsgSrv::NewFile(path, all_files) => {
                let (msg0, msg1) = tokio::join!(
                    tx4.send(MsgSrv::NewFile(path.clone(), all_files.clone())),
                    tx6.send(MsgSrv::NewFile(path, all_files))
                );

                msg0.unwrap();
                msg1.unwrap();
            }
            MsgSrv::Exit() => {
                let (msg0, msg1) = tokio::join!(tx4.send(MsgSrv::Exit()), tx6.send(MsgSrv::Exit()));

                // ignore errors
                msg0.ok();
                msg1.ok();
                break;
            }
        }
    }

    log::debug!("Exited silently");
}
