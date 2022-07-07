use std::{collections::HashMap, str::FromStr, fmt::Debug};

pub struct Arguments(HashMap<String, Option<String>>, Vec<String>);

pub fn parse() -> Arguments {
	let mut named = HashMap::new();
	let mut positional = Vec::new();

	let args = std::env::args();
	let mut current_name: Option<String> = None;
	for a in args {
		if a.starts_with("--") {
			if let Some(s) = &current_name {
				named.insert(s.to_owned(), None);
			}

			current_name = Some(a);
		} else if let Some(s) = &current_name {
			named.insert(s.to_owned(), Some(a));
		} else {
			positional.push(a);
		}
	}

	Arguments(named, positional)
}

impl Arguments {
	pub fn get_named<T: FromStr>(&self, name: &str) -> Option<T> where <T as FromStr>::Err: Debug {
		self.0.get(name)?
			.as_ref()
			.map(|v| T::from_str(&v).expect("Failed to parse argument"))
	}

	pub fn get_positional<T: FromStr>(&self, i: usize) -> Option<T> where <T as FromStr>::Err: Debug {
		self.1.get(i)
			.map(|v| T::from_str(&v).expect("Failed to parse argument"))
	}
}