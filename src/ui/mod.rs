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
use std::{
    cmp::Ordering,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use maud::{html, Markup, PreEscaped, DOCTYPE};
use regex::Regex;

use crate::builder::BuiltFile;

fn hash(s: &'static str) -> u64 {
    let mut hasher = DefaultHasher::default();
    s.hash(&mut hasher);

    hasher.finish()
}

fn render_head(title: &str) -> Markup {
    let css = format!(
        "{}{}",
        include_str!("./style.css"),
        include_str!("./prism.css")
    );

    let css = css.replace('\n', "");
    let whitespace = Regex::new(r"\s+").unwrap();
    let css = whitespace.replace_all(css.as_str(), " ");
    html! {
        meta charset="utf-8";
        title { (title) }
        meta name="description" content=(format!("{}", title));
        script src=(format!("/.rsc/ws.js?{}", hash(include_str!("./ws.js")))) defer {
        }
        script src=(format!("/.rsc/prism.js?{}", hash(include_str!("./prism.js")))) defer {
        }
        style {
            (PreEscaped(css))
        }
    }
}

#[inline]
fn determine_visible_file<'a>(file: &'a Vec<&str>, depth: usize) -> &'a [&'a str] {
    &file[depth..]
}

#[inline]
fn render_sidebar_file(file: &Vec<&str>, depth: usize) -> Markup {
    let visible_file = determine_visible_file(file, depth);
    let visible_file = visible_file.join("/");
    let href = format!(
        "/{}",
        file.iter()
            .map(|part| urlencoding::encode(part).to_string())
            .collect::<Vec<String>>()
            .join("/")
    );
    html! {
        div class="file" {
            a href=(href) {
                (visible_file)
            }
        }
    }
}

/// Splits given files in their directory at depth, if they have an directory at the given depth.
fn split_dirs<'a>(
    files: &'a [&'a Vec<&'a str>],
    depth: usize,
) -> Vec<(&'a str, &'a [&'a Vec<&'a str>])> {
    let mut dirs: Vec<(&str, &[&Vec<&str>])> = Vec::new();
    let mut last_dir = None;
    let mut start_index = 0;
    let mut current_index = 0;
    for &file in files {
        let visible_file = determine_visible_file(file, depth);
        if visible_file.len() > 1 {
            // Path has at least one directory?
            if last_dir.is_none() {
                last_dir = Some(visible_file[0]);
            } else if *last_dir.unwrap() != *visible_file[0] {
                for file in &files[start_index..current_index] {
                    assert_eq!(*last_dir.unwrap(), *file[depth]);
                }
                dirs.push((last_dir.unwrap(), &files[start_index..current_index]));
                start_index = current_index;
                last_dir = Some(visible_file[0]);
            }
        } else {
            // No directory (anymore), start searching at next index and, if appropriate, push the
            // last discovered directory to the result
            if let Some(old_last_dir) = last_dir {
                dirs.push((old_last_dir, &files[start_index..current_index]));
                start_index = current_index + 1;
                last_dir = None;
            } else {
                start_index = current_index + 1;
            }
        }

        current_index += 1;
    }

    if let Some(last_dir) = last_dir {
        dirs.push((last_dir, &files[start_index..current_index]));
    }

    dirs
}

#[inline]
fn render_sidebar_dir(files: &[&Vec<&str>], depth: usize) -> Markup {
    let dirs = split_dirs(files, depth);

    let files: Vec<&Vec<&str>> = files
        .iter()
        .filter(|&f| f.len() == depth + 1)
        .copied()
        .collect();

    html! {
        @for file in files {
            (render_sidebar_file(file, depth))
        }
        @for (dir, dirs) in dirs {
            div class="dir" {
                div class="dir-name" {
                    (format!("/{}", dir))
                }

                (render_sidebar_dir(dirs, depth + 1))
            }
        }
    }
}

pub fn render_sidebar(files: &[String]) -> Markup {
    let mut files: Vec<&String> = files.iter().collect();
    files.sort_by(|&a, &b| -> Ordering {
        //let cnt_dir_a = a.matches("/").count();
        //let cnt_dir_b = b.matches("/").count();

        a.cmp(b)
    });

    let files: Vec<Vec<&str>> = files
        .iter()
        .map(|f| f.as_str())
        .map(|f| f[1..].split('/').collect())
        .collect();

    let files: Vec<&Vec<&str>> = files.iter().collect();

    render_sidebar_dir(&files[..], 0)
}

/// Renders the page's main contents
pub fn render_contents(contents: Contents) -> Markup {
    html! {
        main {
            @match contents {
                Contents::Html(html_contents) => div {
                    div id="built-content" {
                        (PreEscaped(html_contents.contents.as_str()))
                    }

                    div id="words" {
                        "Words: " span id="word-count" {
                            (html_contents.word_count)
                        }
                    }
                },
                Contents::Text(text) =>  pre { (text) },
                Contents::NotFound() => "404 - Not found"
            }
    }
    }
}

pub enum Contents<'a> {
    Html(&'a BuiltFile),
    Text(&'a str),
    NotFound(),
}

/// Renders just the body
fn render_body(contents: Contents, files: &[String]) -> Markup {
    html! {
        nav id="sidebar" {
            (render_sidebar(files))
        }
        div id="contents" {
            (render_contents(contents))
        }
        a href="/.license" title="License" {
            div id="info" {
                (md_icons::filled::maud_icon_info())
            }
        }
    }
}

/// Renders to whole HTML Page
pub fn render_page(title: &str, contents: Contents, files: &[String]) -> Markup {
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
