use std::io::Write;
use std::{collections::HashSet, path::PathBuf};
use std::time;
use std::fs;
use rusqlite::{Connection, params};
use sha2::{Sha256, Digest};

pub fn create_indices(url: &str, tags: &HashSet<String>) {
	let matches = crate::REG_URL_PARSE.captures(url)
		.expect("Invalid URL passed to create_indices");
	
	let protocol = &matches[1];
	let host = &matches[2];
	let pathname = matches.get(3)
		.map(|v| v.as_str())
		.unwrap_or("/");

	let conn = Connection::open("index.db")
		.expect("Failed to open database");
	
	// TODO: Reindexing pages
	let now = time::SystemTime::now()
		.duration_since(time::UNIX_EPOCH)
		.expect("Failed to get current time")
		.as_secs();
	
	conn.execute("
		INSERT INTO pages (
			protocol, host, pathname, last_indexed
		) VALUES (?, ?, ?, ?)
	", params![ protocol, host, pathname, now ]).expect("Failed to index page");

	let page_id: u64 = conn.query_row("
		SELECT id
			FROM pages
		WHERE protocol = ?
			AND host = ?
			AND pathname = ?
	", params![ protocol, host, pathname ], |row| row.get(0))
	.expect("Failed to select page ID");

	for t in tags.iter() {
		create_index(page_id, t);
	}
}

pub fn create_index(page_id: u64, tag: &str) {
	let mut hash = Sha256::new();
	hash.update(tag);
	let hash = bytes_to_hex(&hash.finalize());
	let prefix = &hash[..2];
	let filename = &hash[2..];

	let mut indices = PathBuf::from("indices");
	indices.push(prefix);
	if !indices.exists() {
		fs::create_dir_all(&indices)
			.expect("Failed to create output directory");
	}

	indices.push(filename);

	let mut file = fs::File::options()
		.append(true)
		.create(true)
		.open(indices)
		.expect("Failed to open output file");
	
	let buf = page_id.to_be_bytes();
	file.write_all(&buf).expect("Failed to write to output file");
}

fn bytes_to_hex(bytes: &[u8]) -> String {
	let mut buf = String::with_capacity(bytes.len() * 2);
	for b in bytes {
		buf.push_str(&format!("{:x}", b));
	}

	buf
}