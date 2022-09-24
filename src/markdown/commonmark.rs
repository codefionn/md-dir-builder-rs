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

use pulldown_cmark::{html, Options, Parser};

pub struct CommonMarkParser {}

impl CommonMarkParser {
    fn create_options() -> Options {
        // Enable some default quality of live improvements
        let mut options = Options::empty();
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_TABLES);

        options
    }
}

impl Default for CommonMarkParser {
    fn default() -> Self {
        CommonMarkParser {}
    }
}

impl MarkdownParser for CommonMarkParser {
    fn parse_to_html(&mut self, markdown: &str) -> String {
        let parser = Parser::new_ext(markdown, Self::create_options());

        // Write to String buffer.
        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);

        html_output
    }
}
