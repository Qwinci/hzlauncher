use iced::futures::{stream, StreamExt};

pub struct Downloader<I> {
	client: reqwest::Client,
	downloads: Vec<(I, String)>,
	pub parallel: usize
}

impl<I> Downloader<I> {
	pub const fn new(client: reqwest::Client) -> Self {
		Self { client, downloads: Vec::new(), parallel: 8 }
	}

	pub async fn download_all(&mut self) -> Vec<(I, Result<Vec<u8>, reqwest::Error>)> {
		let downloads = std::mem::take(&mut self.downloads);
		let s: &Downloader<I> = self;
		let results: Vec<_> = stream::iter(downloads)
			.map(|(id, url)| async move {
				let res = match s.client.get(url).send().await {
					Ok(res) => match res.bytes().await {
						Ok(bytes) => Ok(bytes.to_vec()),
						Err(err) => Err(err)
					},
					Err(err) => Err(err)
				};
				(id, res)
			}).buffer_unordered(self.parallel)
			.collect().await;
		results
	}

	pub async fn download_one(&self, url: &str) -> Result<Vec<u8>, reqwest::Error> {
		Ok(self.client.get(url).send().await?.bytes().await?.to_vec())
	}

	pub fn add_download(&mut self, id: I, url: String) {
		self.downloads.push((id, url));
	}
}
