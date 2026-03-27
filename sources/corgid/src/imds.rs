//! EC2 Instance Metadata Service (IMDS) client.
//!
//! Retrieves IAM credentials from IMDS using IMDSv2 session tokens.

use crate::error::{self, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use snafu::ResultExt;

const IMDS_BASE: &str = "http://169.254.169.254";
const IMDS_TOKEN_PATH: &str = "/latest/api/token";
const IMDS_CREDENTIALS_PATH: &str =
    "/latest/meta-data/identity-credentials/ec2/security-credentials/ec2-instance";

/// Client for fetching metadata from IMDS.
pub struct ImdsClient {
    client: Client,
    token: String,
}

impl ImdsClient {
    /// Creates a new IMDS client with a session token.
    pub fn new() -> Result<Self> {
        let client = Client::new();
        let token = client
            .put(format!("{}{}", IMDS_BASE, IMDS_TOKEN_PATH))
            .header("X-aws-ec2-metadata-token-ttl-seconds", "300")
            .send()
            .context(error::ImdsCredentialsSnafu)?
            .text()
            .context(error::ImdsCredentialsSnafu)?;
        Ok(Self { client, token })
    }

    /// Fetches a metadata path from IMDS.
    pub fn get(&self, path: &str) -> Result<String> {
        self.client
            .get(format!("{}{}", IMDS_BASE, path))
            .header("X-aws-ec2-metadata-token", &self.token)
            .send()
            .context(error::ImdsCredentialsSnafu)?
            .text()
            .context(error::ImdsCredentialsSnafu)
    }
}

/// IAM credentials retrieved from IMDS for signing API requests.
#[derive(Deserialize)]
pub struct Credentials {
    #[serde(rename = "AccessKeyId")]
    pub access_key_id: String,
    #[serde(rename = "SecretAccessKey")]
    pub secret_access_key: String,
    #[serde(rename = "Token")]
    pub token: String,
}

impl Credentials {
    /// Retrieves Instance Identity Role credentials from IMDS.
    pub fn from_imds() -> Result<Self> {
        let imds = ImdsClient::new()?;
        let creds_json = imds.get(IMDS_CREDENTIALS_PATH)?;
        serde_json::from_str(&creds_json).context(error::ParseCredentialsSnafu)
    }
}
