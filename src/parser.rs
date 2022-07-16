use std::collections::HashSet;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static!{ static ref REG_INNER_TEXT: Regex = Regex::new(r"\w+").unwrap(); }
lazy_static!{ static ref REG_STRIP_SPECIALS: Regex = Regex::new(r"[^\s\w]+").unwrap(); }
lazy_static!{ static ref REG_META: Regex = Regex::new(r"<meta.+?>").unwrap(); }
lazy_static!{ static ref REG_META_CONTENT: Regex = Regex::new("content=\"(.*)\"").unwrap(); }
lazy_static!{ static ref REG_WORDS: Regex = Regex::new(r"\w+").unwrap(); }
lazy_static!{ static ref REG_URL: Regex = Regex::new(r"https?://[A-Za-z0-9-._~:/?#&\[\]@!$'()*+,;=%]+").unwrap(); }
lazy_static!{ static ref REG_LINK: Regex = Regex::new(r"/[A-Za-z0-9-._~:/?&@!$+=%]+(\s*>)?").unwrap(); }

#[derive(Debug)]
pub struct Document {
	pub links: HashSet<String>,
	pub tags: HashSet<String>,
}

impl Document {
	pub fn parse(source: &str, url: &str) -> Self {
		let line_stripped = source
			.to_lowercase()
			.replace("\r", " ")
			.replace("\n", " ");

		let text_tags = parse_tags(&line_stripped);
		let meta_tags = parse_meta(&line_stripped);
		let mut tags = text_tags;
		tags.extend(meta_tags);

		let links = parse_links(source, url);
		Self {
			links,
			tags,
		}
	}
}

fn parse_links(source: &str, url: &str) -> HashSet<String> {
	let url_parse = crate::REG_URL_PARSE.captures(url).expect("Invalid URL passed to parse_links");
	let protocol = &url_parse[1];
	let host = &url_parse[2];

	let mut res = HashSet::new();
	let urls = REG_URL.find_iter(source);
	for u in urls {
		res.insert(u.as_str().to_owned());
	}

	let links = REG_LINK.find_iter(source);
	for l in links {
		let ls = l.as_str();
		if ls.starts_with("//") || ls.ends_with(">") {
			continue;
		}

		res.insert(format!("{}://{}{}", protocol, host, ls));
	}

	res
}

fn parse_meta(source: &str) -> HashSet<String> {
	let lower = source.to_lowercase();
	let metas = REG_META.find_iter(&lower);
	let mut res = HashSet::new();
	for m in metas {
		if m.as_str().contains("name=\"keywords\"") {
			if let Some(keywords) = REG_META_CONTENT.captures(m.as_str()) {
				let keywords = keywords[1].split(",");
				for k in keywords {
					res.insert(k.trim().to_owned());
				}
			}
		}
	}

	res
}

fn parse_tags(source: &str) -> HashSet<String> {
	let mut tags = HashSet::new();
	let inner_text = REG_INNER_TEXT.find_iter(source);
	for m in inner_text {
		let text = m.as_str();
		let text = match htmlescape::decode_html(text) {
			Ok(v) => v,
			Err(_) => text.to_owned(),
		};

		let text = REG_STRIP_SPECIALS.replace_all(&text, "");
		let words = REG_WORDS.find_iter(&text);
		for w in words {
			tags.insert(w.as_str().to_owned());
		}
	}

	tags
}