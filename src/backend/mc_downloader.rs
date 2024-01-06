use crate::backend::Downloader;
use crate::model::all_versions;

const VERSIONS_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

pub struct McDownloader {
	downloader: Downloader<usize>
}

impl McDownloader {
	pub const fn new(client: reqwest::Client) -> Self {
		Self { downloader: Downloader::new(client) }
	}

	pub fn add_download(&mut self, url: String, id: usize) {
		self.downloader.add_download(id, url)
	}

	pub async fn download_all(&mut self) -> Vec<(usize, reqwest::Result<Vec<u8>>)> {
		self.downloader.download_all().await
	}

	pub async fn download_one(&self, url: &str) -> Result<Vec<u8>, reqwest::Error> {
		self.downloader.download_one(url).await
	}

	pub async fn download_versions(&self) -> reqwest::Result<all_versions::Versions> {
		let data = self.downloader.download_one(VERSIONS_URL).await?;
		let versions: all_versions::Versions = serde_json::from_slice(&data).unwrap();
		Ok(versions)
	}
}
