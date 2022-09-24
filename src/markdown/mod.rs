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
mod cache;
mod commonmark;
pub use commonmark::CommonMarkParser;

/// Generic for parsing markdown to html
pub trait MarkdownParser {
    /// Returns HTML parsed from the input `markdown`
    ///
    /// # Arguments
    ///
    /// * `markdown`: Input markdown (CommonMark)
    fn parse_to_html(&mut self, markdown: &str) -> String;
}
