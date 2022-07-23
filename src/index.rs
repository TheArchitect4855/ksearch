use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::Display;
use std::io::{Write, Read, Seek, SeekFrom};
use std::path::Path;
use std::{collections::HashSet, path::PathBuf};
use std::time;
use std::fs;
use rusqlite::{Connection, params, OptionalExtension};
use sha2::{Sha256, Digest};

pub struct QueryResult {
	pub url: String,
	pub rank: u64,
	pub hits: usize,
}

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
	
	let now = time::SystemTime::now()
		.duration_since(time::UNIX_EPOCH)
		.expect("Failed to get current time")
		.as_secs();

	let mut content_hash = Sha256::new();
	for tag in tags {
		content_hash.update(tag);
	}

	let content_hash = bytes_to_hex(&content_hash.finalize());
	let page_id: Option<u64> = conn.query_row("
		SELECT id
		FROM pages
		WHERE protocol = ?
			AND host = ?
			AND pathname = ?
			OR content_hash = ?
	", params![ protocol, host, pathname, content_hash ], |row| row.get(0))
		.optional()
		.expect("Failed to select page ID");
	
	let page_id = if let Some(page_id) = page_id {
		conn.execute("
			UPDATE pages
			SET last_indexed = ?
			WHERE id = ?
		", params![ now, page_id ])
		.expect("Failed to update page data");

		page_id
	} else {
		conn.execute("
			INSERT INTO pages (
				protocol, host, pathname, last_indexed, content_hash
			) VALUES (?, ?, ?, ?, ?)
		", params![ protocol, host, pathname, now, content_hash	 ])
		.expect("Failed to insert page data");

		conn.query_row("
			SELECT id
				FROM pages
			WHERE protocol = ?
				AND host = ?
				AND pathname = ?
		", params![ protocol, host, pathname ], |row| row.get(0))
		.expect("Failed to select page ID")
	};

	for t in tags.iter() {
		create_index(page_id, t);
	}

	Ok(page_id)
}

pub fn create_index(page_id: u64, tag: &str) {
	let indices = get_tag_index(tag);
	let exists = indices.exists();
	let mut file = fs::File::options()
		.append(true)
		.create(true)
		.read(true)
		.open(&indices)
		.expect("Failed to open output file");
	
	let mut write_to = 0;
	let mut file_size = 0;
	if exists {
		let meta = file.metadata().expect("Failed to get file metadata");
		file_size = meta.len();
		let num_indices = file_size / 8;

		let mut seek_to = num_indices / 2;
		let mut partition = (0, num_indices);
		let mut buffer = [0; 8];
		write_to = loop {
			file.seek(SeekFrom::Start(seek_to * 8)).expect("Failed to seek file");
			file.read_exact(&mut buffer).expect("Failed to read from file");
			
			let pid = u64::from_be_bytes(buffer);
			if pid > page_id && partition.1 != seek_to {
				// Seek back
				partition.1 = seek_to;
			} else if pid < page_id && partition.0 != seek_to {
				// Seek forwards
				partition.0 = seek_to;
			} else if pid == page_id {
				// If we find our page ID, then this is already indexed
				return;
			}

			if partition.0 + 1 == partition.1 {
				println!("partition: {:?}\tpid: {}\tpage_id: {}", partition, pid, page_id);
				if pid > page_id {
					break partition.0;
				} else {
					break partition.1;
				}
			}

			let min = partition.1 - partition.0;
			seek_to = (min / 2) + partition.0;
		} * 8;
	}

	if file_size == 0 {
		let buf = page_id.to_be_bytes();
		file.write_all(&buf).expect("Failed to write index");
		return;
	}

	file.seek(SeekFrom::Start(write_to)).expect("Failed to seek file");
	let mut ahead = [0; 8];
	let mut write = page_id.to_be_bytes();
	while write_to < file_size {
		file.seek(SeekFrom::Start(write_to)).expect("Failed to seek file");
		file.read_exact(&mut ahead).expect("Failed to read index");
		file.seek(SeekFrom::Current(-8)).expect("Failed to seek file");
		file.write_all(&write).expect("Failed to write index");
		write = ahead;
		write_to += 8;
	}

	file.write_all(&write).expect("Failed to write index");
	if cfg!(debug_assertions) {
		validate_index(&indices);
	}
}

pub fn query(tags: &HashSet<String>) -> Box<[QueryResult]> {
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
		if a_tags < b_tags {
			Ordering::Greater
		} else if a_tags > b_tags {
			Ordering::Less
		} else if a_rank < b_rank {
			Ordering::Greater
		} else if a_rank > b_rank {
			Ordering::Less
		} else {
			Ordering::Equal
		}
	});

	rows.iter().map(|v| QueryResult {
		url: v.1.to_owned(),
		rank: *page_ranks.get(&v.0).unwrap(),
		hits: *tag_counts.get(&v.0).unwrap(),
	}).collect::<Vec<QueryResult>>().into_boxed_slice()
}

pub fn validate_index(path: &Path) {
	let mut file = fs::File::open(path).expect("Failed to open index");
	let mut last = 0;
	let mut buf = [0; 8];
	while let Ok(_) = file.read_exact(&mut buf) {
		let n = u64::from_be_bytes(buf);
		if last >= n {
			panic!("Index validation failed");
		}

		last = n;
	}
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

impl Display for QueryResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (rank {}, {} hits)", self.url, self.rank, self.hits)
    }
}