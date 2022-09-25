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
use tokio::sync;

#[derive(PartialEq, Eq, Debug)]
pub enum MsgSrv {
    File(/* path: */ String, /* content: */ String),
    NewFile(/* path: */ String, /* all_files: */ Vec<String>),
    Exit(),
}

#[derive(Debug)]
pub enum MsgBuilder {
    File(
        /* path: */ String,
        /* result: */ sync::oneshot::Sender<(Option<String>, /* all_files: */ Vec<String>)>,
    ),
    Exit(),
}

#[derive(Debug)]
pub enum MsgInternalBuilder {
    FileCreated(/* path: */ String),
    FileModified(/* path: */ String),
    FileDeleted(/* path: */ String),
    Exit(),
}
