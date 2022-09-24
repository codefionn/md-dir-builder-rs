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
use std::cmp::Ordering;

use maud::{html, Markup, PreEscaped, DOCTYPE};

fn render_head(title: &str) -> Markup {
    let css = include_str!("./style.css");
    let css = css.replace("\n", "");
    html! {
        meta charset="utf-8";
        title { (title) }
        meta name="description" content=(format!("{}", title));
        script src="/.rsc/ws.js" defer {
        }
        style {
            (PreEscaped(css))
        }
    }
}

fn render_sidebar(files: &[String]) -> Markup {
    let mut files: Vec<&String> = files.iter().collect();
    files.sort_by(|&a, &b| -> Ordering {
        //let cnt_dir_a = a.matches("/").count();
        //let cnt_dir_b = b.matches("/").count();

        a.cmp(b)
    });

    html! {
        @for file in files {
            div id="file" {
                a href=(file) {
                    (file)
                }
            }
        }
    }
}

/// Renders the page's main contents
fn render_contents<'a>(contents: Contents<'a>) -> Markup {
    html! {
        main {
            @match contents {
                Contents::Html(html_contents) => (PreEscaped(html_contents)),
                Contents::Text(text) =>  (text),
                Contents::NotFound() => "404 - Not found"
            }
    }
    }
}

pub enum Contents<'a> {
    Html(&'a str),
    Text(&'a str),
    NotFound(),
}

/// Renders just the body
fn render_body<'a>(contents: Contents<'a>, files: &[String]) -> Markup {
    html! {
        nav id="sidebar" {
            (render_sidebar(files))
        }
        div id="contents" {
            (render_contents(contents))
        }
    }
}

/// Renders to whole HTML Page
pub fn render_page<'a>(title: &str, contents: Contents<'a>, files: &[String]) -> Markup {
    html! {
        (DOCTYPE)
        html {
            head {
                (render_head(title))
            }
            body {
                (render_body(contents, files))
            }
        }
    }
}
