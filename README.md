# md-dir-builder

A markdown directory server for a cool editing experience with any editor.

This project also shows how to implement concurrent systems in Rust (with ``tokio``).

## Clone & run

```sh
git clone https://github.com/codefionn/md-dir-builder-rs
cd md-dir-builder-rs
cargo run -- -p 8082
```

Get help:

```sh
cargo run -- --help
```

## Markdown parsing

Currently markdown parsing is done with the ``pulldown-cmark`` library (like mdBook).

## TODO

* Watch newly created directories
* Expandable directories in sidebar not just the file path
* Handle connection losses to server
* Add **About** section with option to show GPL license
* Adjustable IP-address (currently ``127.0.0.1`` and ``::1`` are used)
* Document the stuff
* Syntax highlighting for code
