use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Latest {
	pub release: String,
	pub snapshot: String
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Version {
	pub id: String,
	#[serde(rename = "type")]
	pub t: String,
	pub url: String,
	pub time: String,
	#[serde(rename = "releaseTime")]
	pub release_time: String,
	pub sha1: String,
	#[serde(rename = "complianceLevel")]
	pub compliance_level: usize
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Versions {
	pub latest: Latest,
	pub versions: Vec<Version>
}
