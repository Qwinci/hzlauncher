use std::sync::Arc;
use tokio::sync::Mutex;
use crate::backend::{McDownloader, McManager, McResult};

#[derive(Clone)]
pub struct UiManagerWrapper {
	pub inner: Arc<Mutex<McManager>>
}

impl UiManagerWrapper {
	pub fn new(mc_downloader: McDownloader) -> Self {
		Self { inner: Arc::new(Mutex::new(McManager::new(mc_downloader))) }
	}

	pub async fn load_versions(self) -> McResult<()> {
		self.inner.lock().await.load_versions().await
	}

	pub async fn play_version(self, version: String) -> McResult<()> {
		self.inner.lock().await.play_version(&version).await
	}
}
