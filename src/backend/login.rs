use std::fs::{read_to_string, write};
use std::path::Path;
use std::time::{Duration, SystemTime};
use litcrypt2::lc_env;
use oauth2::basic::{BasicClient, BasicTokenResponse};
use oauth2::{AuthUrl, ClientId, DeviceAuthorizationUrl, HttpRequest, HttpResponse, RedirectUrl, RefreshToken, Scope, StandardDeviceAuthorizationResponse, TokenResponse, TokenUrl};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use crate::model::{Account, McCredentials, MsCredentials};

const AUTH_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";
const DEVICE_CODE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const REDIRECT_URL: &str = "https://login.microsoftonline.com/common/oauth2/nativeclient";

const XBOX_AUTH_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XBOX_SECURE_AUTH_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";

const MC_AUTH_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

pub const ACCOUNT_FILE: &str = "data/account.toml";

const fn env_checker() {
	env!("CLIENT_ID");
}

const USELESS: () = env_checker();

#[derive(Serialize)]
struct XboxLoginProperties {
	#[serde(rename = "AuthMethod")]
	pub auth_method: String,
	#[serde(rename = "SiteName")]
	pub site_name: String,
	#[serde(rename = "RpsTicket")]
	pub rps_ticket: String
}

#[derive(Serialize)]
struct XboxLoginRequest {
	#[serde(rename = "Properties")]
	pub properties: XboxLoginProperties,
	#[serde(rename = "RelyingParty")]
	pub relying_party: String,
	#[serde(rename = "TokenType")]
	pub token_type: String
}

#[derive(Deserialize)]
struct XboxLoginXui {
	uhs: String
}

#[derive(Deserialize)]
struct XboxLoginDisplayClaims {
	xui: Vec<XboxLoginXui>
}

#[derive(Deserialize)]
struct XboxLoginResponse {
	#[serde(rename = "Token")]
	token: String,
	#[serde(rename = "DisplayClaims")]
	display_claims: XboxLoginDisplayClaims
}

#[derive(Serialize)]
struct XboxSecureTokenProperties {
	#[serde(rename = "SandboxId")]
	sandbox_id: String,
	#[serde(rename = "UserTokens")]
	user_tokens: Vec<String>
}

#[derive(Serialize)]
struct XboxSecureTokenRequest {
	#[serde(rename = "Properties")]
	properties: XboxSecureTokenProperties,
	#[serde(rename = "RelyingParty")]
	relying_party: String,
	#[serde(rename = "TokenType")]
	token_type: String
}

#[derive(Deserialize)]
struct XboxSecureTokenResponse {
	#[serde(rename = "Token")]
	token: String
}

#[derive(Serialize)]
struct MinecraftLoginRequest {
	#[serde(rename = "identityToken")]
	identity_token: String
}

#[derive(Deserialize)]
struct MinecraftLoginResponse {
	access_token: String,
	expires_in: u64
}

#[derive(Deserialize)]
struct MinecraftProfileResponse {
	id: String,
	name: String
}

struct XboxResponses {
	login: XboxLoginResponse,
	token: XboxSecureTokenResponse
}

pub async fn custom_async_http_client(
	client: &reqwest::Client,
	request: HttpRequest,
) -> Result<HttpResponse, oauth2::reqwest::Error<reqwest::Error>> {
	let mut request_builder = client
		.request(request.method, request.url.as_str())
		.body(request.body);
	for (name, value) in &request.headers {
		request_builder = request_builder.header(name.as_str(), value.as_bytes());
	}
	let request = request_builder.build().map_err(oauth2::reqwest::Error::Reqwest)?;

	let response = client.execute(request).await.map_err(oauth2::reqwest::Error::Reqwest)?;

	let status_code = response.status();
	let headers = response.headers().to_owned();
	let chunks = response.bytes().await.map_err(oauth2::reqwest::Error::Reqwest)?;
	Ok(HttpResponse {
		status_code,
		headers,
		body: chunks.to_vec(),
	})
}

pub async fn ms_code_login(http_client: reqwest::Client) -> Result<StandardDeviceAuthorizationResponse, String> {
	let client = BasicClient::new(
		ClientId::new(lc_env!("CLIENT_ID")),
		None,
		AuthUrl::new(AUTH_URL.to_string()).unwrap(),
		Some(TokenUrl::new(TOKEN_URL.to_string()).unwrap()),
	)
		.set_device_authorization_url(DeviceAuthorizationUrl::new(DEVICE_CODE_URL.to_string()).unwrap())
		.set_redirect_uri(RedirectUrl::new(REDIRECT_URL.to_string()).unwrap());

	let res: Result<StandardDeviceAuthorizationResponse, _> = client.exchange_device_code().unwrap()
		.add_scope(Scope::new("offline_access".to_string()))
		.add_scope(Scope::new("XboxLive.signin".to_string()))
		.add_scope(Scope::new("XboxLive.offline_access".to_string()))
		.request_async(|req| custom_async_http_client(&http_client, req)).await.map_err(|err| err.to_string());
	res
}

async fn xbox_login(http_client: &reqwest::Client, ms_access_token: &str) -> Result<XboxResponses, reqwest::Error> {
	let req = XboxLoginRequest {
		properties: XboxLoginProperties {
			auth_method: "RPS".to_string(),
			site_name: "user.auth.xboxlive.com".to_string(),
			rps_ticket: format!("d={}", ms_access_token)
		},
		relying_party: "http://auth.xboxlive.com".to_string(),
		token_type: "JWT".to_string()
	};
	let res = http_client.request(Method::POST, XBOX_AUTH_URL)
		.body(serde_json::to_vec(&req).unwrap())
		.header("x-xbl-contract-version", 1)
		.send().await?.bytes().await?;
	let login_res: XboxLoginResponse = serde_json::from_slice(&res).unwrap();

	let req = XboxSecureTokenRequest {
		properties: XboxSecureTokenProperties {
			sandbox_id: "RETAIL".to_string(),
			user_tokens: vec![login_res.token.clone()]
		},
		relying_party: "rp://api.minecraftservices.com/".to_string(),
		token_type: "JWT".to_string()
	};
	let res = http_client.request(Method::POST, XBOX_SECURE_AUTH_URL)
		.body(serde_json::to_vec(&req).unwrap())
		.send().await?.bytes().await?;
	let token_res: XboxSecureTokenResponse = serde_json::from_slice(&res).unwrap();

	Ok(XboxResponses {
		login: login_res,
		token: token_res
	})
}

async fn mc_login(http_client: &reqwest::Client, ms_creds: &MsCredentials)
	-> Result<McCredentials, reqwest::Error> {
	let req = MinecraftLoginRequest {
		identity_token: format!("XBL3.0 x={};{}", ms_creds.user_hash, ms_creds.xsts_token)
	};
	let res = http_client.request(Method::POST, MC_AUTH_URL)
		.body(serde_json::to_vec(&req).unwrap())
		.send().await?.bytes().await?;
	let res: MinecraftLoginResponse = serde_json::from_slice(&res).unwrap();
	let expires_at = SystemTime::now() + Duration::from_secs(res.expires_in);

	Ok(McCredentials {
		access_token: res.access_token,
		expires_at
	})
}

async fn mc_get_profile(http_client: &reqwest::Client, mc_creds: &McCredentials)
	-> Result<MinecraftProfileResponse, reqwest::Error> {
	let res = http_client.request(Method::GET, MC_PROFILE_URL)
		.header("Authorization", format!("Bearer {}", mc_creds.access_token))
		.send().await?.bytes().await?;
	let res: MinecraftProfileResponse = serde_json::from_slice(&res).unwrap();
	Ok(res)
}

async fn do_full_login_with_token(http_client: &reqwest::Client, token_res: BasicTokenResponse)
                                  -> Result<Account, String> {
	let token = token_res.access_token().secret();
	let expires_in = token_res.expires_in().expect("expected an expiry time");
	let expires_at = SystemTime::now() + expires_in;

	let xbox = xbox_login(&http_client, token).await.map_err(|err| err.to_string())?;

	let ms_creds = MsCredentials {
		access_token: token.clone(),
		refresh_token: token_res.refresh_token().expect("expected a refresh token")
			.secret().clone(),
		expires_at,
		xbox_token: xbox.login.token,
		xsts_token: xbox.token.token,
		user_hash: xbox.login.display_claims.xui[0].uhs.clone()
	};

	let mc_creds = mc_login(&http_client, &ms_creds).await.map_err(|err| err.to_string())?;

	let mc_profile = mc_get_profile(&http_client, &mc_creds).await.map_err(|err| err.to_string())?;

	Ok(Account {
		name: mc_profile.name,
		id: mc_profile.id,
		ms_creds,
		mc_creds
	})
}

pub async fn refresh_ms(http_client: reqwest::Client, acc: Account) -> Result<Account, String> {
	let client = BasicClient::new(
		ClientId::new(lc_env!("CLIENT_ID")),
		None,
		AuthUrl::new(AUTH_URL.to_string()).unwrap(),
		Some(TokenUrl::new(TOKEN_URL.to_string()).unwrap()),
	)
		.set_device_authorization_url(DeviceAuthorizationUrl::new(DEVICE_CODE_URL.to_string()).unwrap())
		.set_redirect_uri(RedirectUrl::new(REDIRECT_URL.to_string()).unwrap());

	let token_res = client.exchange_refresh_token(&RefreshToken::new(acc.ms_creds.refresh_token.clone()))
		.request_async(|req| custom_async_http_client(&http_client, req))
		.await.map_err(|err| err.to_string())?;

	do_full_login_with_token(&http_client, token_res).await
}

pub async fn refresh_mc(http_client: reqwest::Client, mut acc: Account) -> Result<Account, String> {
	let mc_creds = mc_login(&http_client, &acc.ms_creds).await.map_err(|err| err.to_string())?;

	let mc_profile = mc_get_profile(&http_client, &mc_creds).await.map_err(|err| err.to_string())?;

	acc.mc_creds = mc_creds;
	acc.name = mc_profile.name;
	acc.id = mc_profile.id;
	Ok(acc)
}

pub async fn finish_code_login(http_client: reqwest::Client, res: StandardDeviceAuthorizationResponse)
	-> Result<Account, String> {
	let client = BasicClient::new(
		ClientId::new(lc_env!("CLIENT_ID")),
		None,
		AuthUrl::new(AUTH_URL.to_string()).unwrap(),
		Some(TokenUrl::new(TOKEN_URL.to_string()).unwrap()),
	)
		.set_device_authorization_url(DeviceAuthorizationUrl::new(DEVICE_CODE_URL.to_string()).unwrap())
		.set_redirect_uri(RedirectUrl::new(REDIRECT_URL.to_string()).unwrap());

	let token_res = client.exchange_device_access_token(&res)
		.request_async(|req| custom_async_http_client(&http_client, req), tokio::time::sleep, None)
		.await.map_err(|err| err.to_string())?;

	do_full_login_with_token(&http_client, token_res).await
}

pub fn load_account_from_file() -> Option<Account> {
	if let Ok(data) = read_to_string(ACCOUNT_FILE) {
		let acc: Account = toml::from_str(&data).ok()?;
		Some(acc)
	} else {
		None
	}
}

pub fn save_account_to_file(account: &Account) -> std::io::Result<()> {
	let data = toml::to_string(account).unwrap();
	std::fs::create_dir_all(Path::new(ACCOUNT_FILE).parent().unwrap())?;
	write(ACCOUNT_FILE, data)
}
