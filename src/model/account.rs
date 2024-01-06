use std::time::SystemTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsCredentials {
	pub access_token: String,
	pub refresh_token: String,
	pub expires_at: SystemTime,
	pub xbox_token: String,
	pub xsts_token: String,
	pub user_hash: String
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McCredentials {
	pub access_token: String,
	pub expires_at: SystemTime
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
	pub name: String,
	pub id: String,
	pub ms_creds: MsCredentials,
	pub mc_creds: McCredentials
}
