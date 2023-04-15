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
use super::MarkdownParser;
use std::{
    io::{BufReader, BufWriter, Read, Write},
    process::{self, Stdio},
};

pub struct PandocParser {}

impl Default for PandocParser {
    fn default() -> Self {
        PandocParser {}
    }
}

impl MarkdownParser for PandocParser {
    fn parse_to_html(&mut self, markdown: &str) -> String {
        match process::Command::new("pandoc")
            .args(["-f", "markdown"])
            .args(["-t", "html5"])
            .arg("-")
            .args(["-o", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(mut handle) => {
                {
                    let mut stdin = handle.stdin.as_ref().unwrap();
                    let mut writer = BufWriter::new(&mut stdin);
                    writer.write_all(markdown.as_bytes());
                    drop(writer);
                    drop(stdin);
                }

                if handle.wait().is_err() {
                    return "Parsing markdown with pandoc failed".to_string();
                }

                let mut stdout = handle.stdout.unwrap();
                let mut reader = BufReader::new(&mut stdout);
                let mut stdout = String::with_capacity(1024 * 10);
                if reader.read_to_string(&mut stdout).is_err() {
                    return "Parsing markdown with pandoc failed".to_string();
                }

                stdout
            }
            _ => "Parsing markdown with pandoc failed".to_string(),
        }
    }
}
