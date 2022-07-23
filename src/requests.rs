use crate::Error;
use reqwest::Client;

pub async fn get(url: &str) -> Result<String, Error> {
	if !crate::REG_URL_VALIDATE.is_match(url) {
		return Err(Error::from("Invalid URL"));
	}

	let client = Client::builder()
		.danger_accept_invalid_certs(true)
		// .redirect(redirect::Policy::none())
		.user_agent("ksearch-bot")
		.build()
		.expect("Failed to create client");
	
	let req = client.get(url)
		.build()
		.map_err(|e| Error::from(e))?;
	
	let res = client.execute(req).await
		.map_err(|e| Error::from(e))?;
	
	if !res.status().is_success() {
		return Err(Error(
			format!("Server responded with {}", res.status())
		));
	}
	
	let bytes = res.bytes().await
		.map_err(|e| Error::from(e))?
		.into_iter()
		.collect();
	
	let string = String::from_utf8(bytes)
		.map_err(|e| Error::from(e))?;
	
	Ok(string)
}
