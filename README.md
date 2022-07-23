# ksearch
A toy search engine prototype.

This is a prototype I worked on for fun. It can handle indexing and queries, and supports stopwords.
There are a lot of limitations in this engine, but I don't really intend to make this a full-featured
system.

## Installation

[Install Rust](https://www.rust-lang.org/tools/install), then clone this repository.

## Usage

`cargo run -- index [url]` OR `cargo run -- query [query]` (note that multiple words in one query must be wrapped in quotes).
