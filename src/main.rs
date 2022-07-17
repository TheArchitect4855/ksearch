use args::Arguments;
use lazy_static::lazy_static;
use parser::Document;
use regex::Regex;
use rusqlite::{Connection, params};
use std::{fmt::Display, collections::{HashSet, VecDeque}};

mod args;
mod index;
mod parser;
mod requests;

lazy_static!{ pub static ref REG_URL_VALIDATE: Regex = Regex::new(r"^https?://[A-Za-z0-9-._~:/?#&\[\]@!$'()*+,;=%]+$").unwrap(); }
lazy_static!{ pub static ref REG_URL_PARSE: Regex = Regex::new(r"(https?)://([A-Za-z0-9-._~:\[\]@!$'()*+,;=%]+)(/[A-Za-z0-9-._~:/\[\]@!$'()*+,;=?#&%]+)?").unwrap(); }

#[derive(Debug)]
pub struct Error(String);

#[derive(Clone)]
pub struct Link {
	pub from_id: Option<u64>,
	pub to_url: String,
}

#[tokio::main]
async fn main() {
	let arguments = args::parse();
	if let Some(command) = arguments.get_positional::<String>(1) {
		if command == "index" {
			if cfg!(debug_assertions) {
				println!("[DEBUG] DELETING ALL INDEXED PAGES");
				rusqlite::Connection::open("index.db")
					.expect("Couldn't open DB")
					.execute("DELETE FROM pages", rusqlite::params![])
					.expect("Failed to delete pages");
				std::fs::remove_dir_all("indices").expect("Failed to delete indices");
			}

			index(&arguments).await;
		} else if command == "query" {
			query(&arguments).await;
		} else {
			eprintln!("Invalid command {}", command);
		}
	} else {
		eprintln!("Usage: {} [command]", arguments.get_positional::<String>(0).unwrap());
	}
}

async fn index(arguments: &Arguments) {
	if let Some(url) = arguments.get_positional::<String>(2) {
		let mut queue = VecDeque::with_capacity(512);
		queue.push_back(Link {
			from_id: None,
			to_url: url,
		});

		let mut history = HashSet::new();
		while let Some(link) = queue.pop_front() {
			if history.contains(&link.to_url) {
				continue;
			}

			match index_url(&link).await {
				Ok(v) => queue.extend(v.iter().map(|v| v.clone())),
				Err(e) => eprintln!("Error indexing {}: {}", link.to_url, e),
			};

			history.insert(link.to_url);
		}
	} else {
		println!("Usage: {} [url]", arguments.get_positional::<String>(0).unwrap());
	}
}

async fn index_url(link: &Link) -> Result<Box<[Link]>, Error> {
	println!("Indexing {}...", link.to_url);

	let content = requests::get(&link.to_url).await?;
	let document = Document::parse(&content, &link.to_url);
	let page_id = index::create_indices(&link.to_url, &document.tags)?;

	if let Some(from_id) = link.from_id {
		let conn = Connection::open("index.db").expect("Failed to open database");
		if let Err(e) = conn.execute("
			INSERT INTO links (`from`, `to`)
			VALUES (?, ?)
		", params![ from_id, page_id ]) {
			eprintln!("Failed to create link: {}", e);
		}
	}

	let links: Vec<Link> = document.links.iter().map(
		|v| Link {
			from_id: Some(page_id),
			to_url: v.to_string(),
		}
	).collect();

	Ok(links.into_boxed_slice())
}

async fn query(args: &Arguments) {
	let q: String = if let Some(v) = args.get_positional::<String>(2) {
		v.to_lowercase()
	} else {
		eprintln!("Usage: {} query [query string]", args.get_positional::<String>(0).unwrap());
		return;
	};

	let tags = parser::parse_tags(&q);
	let pages = index::get_pages_matching(&tags);
	println!("Pages: {:#?}", pages);
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
