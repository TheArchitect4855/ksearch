use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::{Write, Read};
use std::{collections::HashSet, path::PathBuf};
use std::time;
use std::fs;
use rusqlite::{Connection, params};
use sha2::{Sha256, Digest};

pub fn create_indices(url: &str, tags: &HashSet<String>) -> Result<u64, crate::Error> {
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
	
	if let Err(e) = conn.execute("
		INSERT INTO pages (
			protocol, host, pathname, last_indexed
		) VALUES (?, ?, ?, ?)
	", params![ protocol, host, pathname, now ]) {
		return Err(crate::Error::from(e));
	}

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

	Ok(page_id)
}

pub fn create_index(page_id: u64, tag: &str) {
	let indices = get_tag_index(tag);
	let mut file = fs::File::options()
		.append(true)
		.create(true)
		.open(indices)
		.expect("Failed to open output file");
	
	let buf = page_id.to_be_bytes();
	file.write_all(&buf).expect("Failed to write to output file");
}

pub fn query(tags: &HashSet<String>) -> Box<[String]> {
	let conn = Connection::open("index.db").expect("Failed to open database");
	let mut get_rank = conn.prepare("
		SELECT COUNT(*)
		FROM links
		WHERE `to` = ?
	").unwrap();

	let mut tag_counts: HashMap<u64, usize> = HashMap::new();
	let mut page_ranks: HashMap<u64, u64> = HashMap::new();
	for tag in tags {
		let index = get_tag_index(tag);
		let mut file = match fs::File::options()
			.read(true)
			.open(index) {
				Ok(v) => v,
				Err(_) => {
					eprintln!("Index for {} does not exist", tag);
					continue;
				}
			};
		
		let mut buf = [0; 8];
		while let Ok(_) = file.read_exact(&mut buf) {
			let page_id = u64::from_be_bytes(buf);
			if tag_counts.contains_key(&page_id) {
				*tag_counts.get_mut(&page_id).unwrap() += 1;
			} else {
				tag_counts.insert(page_id, 1);
				let rank = get_rank.query_row(params![ page_id ], |r| r.get::<usize, u64>(0))
					.unwrap_or(0);
				
				page_ranks.insert(page_id, rank);
			}
		}
	}

	let page_ids: Vec<u64> = tag_counts.keys().map(|v| *v).collect();
	let set = page_ids.iter().map(|v| v.to_string()).collect::<Vec<String>>().join(",");
	let conn = Connection::open("index.db").expect("Failed to open database");
	let mut select = conn.prepare(&format!("
		SELECT id, protocol, host, pathname
		FROM pages
		WHERE id IN ({})
	", set)).unwrap();

	let mut rows = select.query_map(params![], |v| {
		Ok((v.get::<usize, u64>(0).unwrap(), format!("{}://{}{}", v.get::<usize, String>(1).unwrap(), v.get::<usize, String>(2).unwrap(), v.get::<usize, String>(3).unwrap())))
	}).unwrap().map(|v| v.unwrap()).collect::<Vec<(u64, String)>>();

	rows.sort_by(|a, b| {
		let a_tags = *tag_counts.get(&a.0).unwrap();
		let b_tags = *tag_counts.get(&b.0).unwrap();
		
		let a_rank = *page_ranks.get(&a.0).unwrap();
		let b_rank = *page_ranks.get(&b.0).unwrap();
		if a_tags > b_tags {
			Ordering::Greater
		} else if a_tags < b_tags {
			Ordering::Less
		} else if a_rank > b_rank {
			Ordering::Greater
		} else if a_rank < b_rank {
			Ordering::Less
		} else {
			Ordering::Equal
		}
	});

	rows.iter().map(|v| v.1.clone()).collect::<Vec<String>>().into_boxed_slice()
}

fn get_tag_index(tag: &str) -> PathBuf {
	let mut hash = Sha256::new();
	hash.update(tag);
	let hash = bytes_to_hex(&hash.finalize());
	
	let prefix = &hash[..2];
	let filename = &hash[2..];

	let mut indices = PathBuf::from("indices");
	indices.push(prefix);
	if !indices.exists() {
		fs::create_dir_all(&indices)
			.expect("Failed to create index directory");
	}

	indices.push(filename);
	indices
}

fn bytes_to_hex(bytes: &[u8]) -> String {
	let mut buf = String::with_capacity(bytes.len() * 2);
	for b in bytes {
		buf.push_str(&format!("{:x}", b));
	}

	buf
}