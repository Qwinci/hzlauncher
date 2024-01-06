use std::fmt::{Display, Formatter};
use std::path::Path;
use aho_corasick::AhoCorasick;
use crate::backend::McDownloader;
use crate::model::{Account, all_versions};

const VERSIONS_FILE: &str = "data/versions.json";
const RESOURCES_URL: &str = "https://resources.download.minecraft.net";

#[derive(Debug, Clone)]
pub enum McError {
	Network(String),
	Fs(String)
}

impl From<reqwest::Error> for McError {
	fn from(value: reqwest::Error) -> Self {
		Self::Network(value.to_string())
	}
}

impl From<tokio::io::Error> for McError {
	fn from(value: tokio::io::Error) -> Self {
		Self::Fs(value.to_string())
	}
}

impl Display for McError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			McError::Network(err) => write!(f, "Network error: {}", err),
			McError::Fs(err) => write!(f, "Filesystem error: {}", err)
		}
	}
}

pub type McResult<T> = Result<T, McError>;

pub struct McManager {
	pub mc_downloader: McDownloader,
	pub versions: Option<all_versions::Versions>,
	pub account: Option<Account>,
	aho: AhoCorasick
}

impl McManager {
	pub fn new(mc_downloader: McDownloader) -> Self {
		let patterns = &[
			"${auth_player_name}",
			"${version_name}",
			"${game_directory}",
			"${assets_root}",
			"${assets_index_name}",
			"${auth_uuid}",
			"${auth_access_token}",
			"${user_type}",
			"${version_type}",
			"${natives_directory}",
			"${launcher_name}",
			"${launcher_version}",
			"${classpath}"
		];
		let aho = AhoCorasick::new(patterns).unwrap();
		Self { mc_downloader, versions: None, account: None, aho }
	}

	pub async fn load_versions(&mut self) -> McResult<()> {
		match tokio::fs::read_to_string(VERSIONS_FILE).await {
			Ok(data) => {
				if let Ok(versions) = serde_json::from_str(&data) {
					return Ok(versions);
				}
			}
			Err(err) if err.kind() != tokio::io::ErrorKind::NotFound => {
				return Err(McError::from(err));
			}
			_ => {}
		}

		let versions = self.mc_downloader.download_versions().await?;
		tokio::fs::create_dir_all(Path::new(VERSIONS_FILE).parent().unwrap()).await?;
		tokio::fs::write(VERSIONS_FILE, serde_json::to_string(&versions).unwrap()).await?;
		self.versions = Some(versions);
		Ok(())
	}

	fn check_rules(rules: Option<&serde_json::Value>) -> bool {
		let rules = match rules {
			Some(rules) => rules,
			None => return true
		};

		let mut allow = true;
		for rule in rules.as_array().unwrap() {
			let action = rule["action"].as_str().unwrap();
			let is = if let Some(os) = rule.get("os") {
				let is_name = if let Some(name) = os.get("name") {
					std::env::consts::OS == name.as_str().unwrap()
				} else {
					true
				};
				let is_arch = if let Some(arch) = os.get("arch") {
					let arch = arch.as_str().unwrap();
					if arch == "x86" && matches!(std::env::consts::ARCH, "x86" | "x86_64") {
						true
					} else {
						std::env::consts::ARCH == arch
					}
				} else {
					true
				};
				is_name && is_arch
			} else if let Some(features) = rule.get("features") {
				let mut is = true;
				for (feature, value) in features.as_object().unwrap() {
					let value = value.as_bool().unwrap();
					if feature == "has_custom_resolution" && value == true {
						// todo
						is = false;
						break;
					} else {
						is = false;
						break;
					}
				}
				is
			} else {
				todo!()
			};
			if (action == "deny" && is) || (action == "allow" && !is) {
				allow = false;
			}
		}
		allow
	}

	fn do_replacements(&self, argument: &str, version: &serde_json::Value, classpath: &str) -> String {
		let acc = self.account.as_ref().unwrap();

		let instance_path = Path::new("data/instance").canonicalize().unwrap();
		let assets_path = Path::new("data/assets").canonicalize().unwrap();
		let natives_path = Path::new("data/natives").canonicalize().unwrap();

		let replace = &[
			acc.name.as_str(),
			version["id"].as_str().unwrap(),
			instance_path.to_str().unwrap(),
			assets_path.to_str().unwrap(),
			version["assets"].as_str().unwrap(),
			acc.id.as_str(),
			acc.mc_creds.access_token.as_str(),
			"mojang",
			version["type"].as_str().unwrap(),
			natives_path.to_str().unwrap(),
			"HZLauncher",
			"1.0",
			classpath
		];
		let res = self.aho.replace_all(argument, replace);
		if res.starts_with('$') {
			String::new()
		} else {
			res
		}
	}

	pub async fn play_version(&mut self, version: &str) -> McResult<()> {
		assert!(self.account.is_some());

		let file_path = format!("data/versions/{}.json", version);
		let version: serde_json::Value = match tokio::fs::read_to_string(&file_path).await {
			Ok(data) => {
				serde_json::from_str(&data).unwrap()
			}
			Err(err) if err.kind() != tokio::io::ErrorKind::NotFound => {
				return Err(McError::from(err));
			}
			_ => {
				let url = &self.versions.as_ref().unwrap().versions.iter().find(|v| v.id == version).unwrap().url;
				let data_vec = self.mc_downloader.download_one(url).await?;
				let data: serde_json::Value = serde_json::from_slice(&data_vec).unwrap();
				tokio::fs::create_dir_all(Path::new(&file_path).parent().unwrap()).await?;
				tokio::fs::write(&file_path, data_vec).await?;
				data
			}
		};

		let mut id = 0;
		let libraries = version["libraries"].as_array().unwrap();
		let mut paths = Vec::with_capacity(libraries.len());
		let mut classpath = String::new();
		for library in libraries {
			let artifact = &library["downloads"]["artifact"];
			let path = artifact["path"].as_str().unwrap();
			let url = artifact["url"].as_str().unwrap();
			let allow = Self::check_rules(library.get("rules"));

			if !allow {
				continue;
			}

			classpath += "data/libraries/";
			if std::env::consts::FAMILY == "unix" {
				classpath += path;
				classpath.push(':');
			} else {
				classpath.push(';');
			}

			if tokio::fs::try_exists(format!("data/libraries/{}", path)).await.is_ok_and(|value| value == true) {
				continue;
			}

			self.mc_downloader.add_download(url.to_string(), id);
			paths.push(path);
			id += 1;
		}

		let results = self.mc_downloader.download_all().await;
		for (id, result) in results {
			let path = paths[id];

			if result.is_err() {
				eprintln!("Failed to download {}", path);
				return Err(McError::from(result.unwrap_err()));
			}

			let full_path = format!("data/libraries/{}", path);
			tokio::fs::create_dir_all(Path::new(&full_path).parent().unwrap()).await?;
			tokio::fs::write(full_path, result.unwrap()).await?;
		}

		tokio::fs::create_dir_all("data/clients").await?;
		let client_file = format!("data/clients/{}.jar", version["id"].as_str().unwrap());
		if !tokio::fs::try_exists(&client_file).await.is_ok_and(|value| value == true) {
			let client_url = version["downloads"]["client"]["url"].as_str().unwrap();
			let client_data = self.mc_downloader.download_one(client_url).await?;
			tokio::fs::write(&client_file, client_data).await?;
		}

		classpath += Path::new(&client_file).canonicalize().unwrap().to_str().unwrap();

		tokio::fs::create_dir_all("data/natives").await?;
		tokio::fs::create_dir_all("data/instance").await?;
		tokio::fs::create_dir_all("data/assets/objects").await?;
		tokio::fs::create_dir_all("data/assets/indexes").await?;
		tokio::fs::create_dir_all("data/assets/virtual/legacy").await?;

		let asset_index = &version["assetIndex"];
		let asset_index_file = format!("data/assets/indexes/{}.json", asset_index["id"].as_str().unwrap());
		let asset_index: serde_json::Value = match tokio::fs::read_to_string(&asset_index_file).await {
			Ok(data) => serde_json::from_str(&data).unwrap(),
			_ => {
				let asset_index_url = asset_index["url"].as_str().unwrap();
				let asset_index_data = self.mc_downloader.download_one(asset_index_url).await?;
				tokio::fs::write(&asset_index_file, &asset_index_data).await?;
				serde_json::from_slice(&asset_index_data).unwrap()
			}
		};

		id = 0;
		let mut paths = Vec::new();
		for (legacy_path, object) in asset_index["objects"].as_object().unwrap() {
			let hash = object["hash"].as_str().unwrap();
			let sub_path = format!("{}/{}", &hash[0..2], hash);
			let path = format!("data/assets/objects/{}", sub_path);
			let legacy_path = format!("data/assets/virtual/legacy/{}", legacy_path);

			if tokio::fs::try_exists(&path).await.is_ok_and(|value| value == true) &&
				tokio::fs::try_exists(&legacy_path).await.is_ok_and(|value| value == true) {
				continue;
			}

			let url = format!("{}/{}", RESOURCES_URL, sub_path);
			self.mc_downloader.add_download(url, id);
			paths.push((sub_path, legacy_path));
			id += 1;
		}

		let results = self.mc_downloader.download_all().await;
		for (id, result) in results {
			let (sub_path, legacy_path) = &paths[id];

			if result.is_err() {
				eprintln!("Failed to download {}", sub_path);
				return Err(McError::from(result.unwrap_err()));
			}

			let full_path = format!("data/assets/objects/{}", sub_path);
			tokio::fs::create_dir_all(Path::new(&full_path).parent().unwrap()).await?;
			tokio::fs::create_dir_all(Path::new(&legacy_path).parent().unwrap()).await?;
			tokio::fs::write(&full_path, result.unwrap()).await?;
			tokio::fs::copy(full_path, legacy_path).await?;
		}

		let mut final_arguments = Vec::new();

		let arguments = &version["arguments"];
		for argument in arguments["jvm"].as_array().unwrap() {
			if !Self::check_rules(argument.get("rules")) {
				continue;
			}
			let value = if let Some(value) = argument.get("value") {
				value
			} else {
				argument
			};
			if let Some(single) = value.as_str() {
				final_arguments.push(self.do_replacements(single, &version, &classpath));
			} else if let Some(multiple) = value.as_array() {
				for argument in multiple {
					final_arguments.push(self.do_replacements(argument.as_str().unwrap(), &version, &classpath));
				}
			} else {
				unreachable!();
			}
		}

		final_arguments.push(version["mainClass"].as_str().unwrap().to_string());

		for argument in arguments["game"].as_array().unwrap() {
			if !Self::check_rules(argument.get("rules")) {
				continue;
			}

			let value = if let Some(value) = argument.get("value") {
				value
			} else {
				argument
			};
			if let Some(single) = value.as_str() {
				final_arguments.push(self.do_replacements(single, &version, &classpath));
			} else if let Some(multiple) = value.as_array() {
				for argument in multiple {
					final_arguments.push(self.do_replacements(argument.as_str().unwrap(), &version, &classpath));
				}
			} else {
				unreachable!();
			}
		}

		let status = tokio::process::Command::new("java")
			.args(final_arguments)
			.spawn()
			.unwrap()
			.wait().await;
		eprintln!("java exited with {:?}", status);

		Ok(())
	}
}
