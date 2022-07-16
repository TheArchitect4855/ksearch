use lazy_static::lazy_static;
use parser::Document;
use regex::Regex;
use std::{fmt::Display, collections::{HashSet, VecDeque}};

mod args;
mod index;
mod parser;
mod requests;

lazy_static!{ pub static ref REG_URL_VALIDATE: Regex = Regex::new(r"^https?://[A-Za-z0-9-._~:/?#&\[\]@!$'()*+,;=%]+$").unwrap(); }
lazy_static!{ pub static ref REG_URL_PARSE: Regex = Regex::new(r"(https?)://([A-Za-z0-9-._~:\[\]@!$'()*+,;=%]+)(/[A-Za-z0-9-._~:/\[\]@!$'()*+,;=?#&%]+)?").unwrap(); }

#[derive(Debug)]
pub struct Error(String);

#[tokio::main]
async fn main() {
	if cfg!(debug_assertions) {
		println!("[DEBUG] DELETING ALL INDEXED PAGES");
		rusqlite::Connection::open("index.db")
			.expect("Couldn't open DB")
			.execute("DELETE FROM pages", rusqlite::params![])
			.expect("Failed to delete pages");
		std::fs::remove_dir_all("indices").expect("Failed to delete indices");
	}

	let arguments = args::parse();
	if let Some(url) = arguments.get_positional::<String>(1) {
		let mut queue = VecDeque::with_capacity(512);
		queue.push_back(url);

		let mut history = HashSet::new();
		while let Some(url) = queue.pop_front() {
			if history.contains(&url) {
				continue;
			}

			match index_url(&url).await {
				Ok(v) => queue.extend(v),
				Err(e) => eprintln!("Error indexing {}: {}", url, e),
			};

			history.insert(url);
		}
	} else {
		println!("Usage: {} [url]", arguments.get_positional::<String>(0).unwrap());
	}
}

async fn index_url(url: &str) -> Result<HashSet<String>, Error> {
	let content = requests::get(url).await?;
	let document = Document::parse(&content, url);
	index::create_indices(url, &document.tags);
	Ok(document.links)
}


impl Error {
	pub fn from<T: ToString>(v: T) -> Self {
		Self(v.to_string())
	}
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
