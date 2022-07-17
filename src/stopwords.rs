use std::collections::HashSet;
use std::io::Read;
use std::sync::Once;
use std::fs::File;

static INIT: Once = Once::new();
static mut STOPWORDS: Option<HashSet<String>> = None;

pub fn filter_stopwords(word: &str) -> bool {
	let stopwords = get_stopwords();
	!stopwords.contains(word)
}

pub fn get_stopwords() -> &'static HashSet<String> {
	unsafe {
		INIT.call_once(load);
		STOPWORDS.as_ref().unwrap()
	}
}

fn load() {
	let mut file = File::open("stop-words.txt")
		.expect("Failed to open stop-words.txt");
	
	let mut contents = Vec::with_capacity(1024);
	let mut buffer = [0; 1024];
	loop {
		let len = file.read(&mut buffer).expect("Failed to read from stopwords file");
		if len == 0 {
			break;
		}

		contents.extend_from_slice(&buffer[..len]);
	}

	let contents = String::from_utf8(contents)
		.expect("Stopwords contains invalid UTF-8");
	
	let mut res = HashSet::new();
	let lines = contents.lines();
	for line in lines {
		res.insert(line.to_owned());
	}

	unsafe {
		STOPWORDS = Some(res);
	}
}