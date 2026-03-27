//! SigV4-signing HTTP client with retry.

use crate::error::{self, Result};
use crate::imds::Credentials;
use crate::metadata::HostMetadata;
use aws_credential_types::Credentials as AwsCredentials;
use aws_sigv4::http_request::{sign, SignableBody, SignableRequest, SigningSettings};
use aws_sigv4::sign::v4;
use log::debug;
use reqwest::blocking::Client;
use snafu::ResultExt;
use std::thread;
use std::time::{Duration, SystemTime};

const SERVICE: &str = "inspector2-telemetry";
const MAX_RETRIES: u32 = 3;

/// HTTP client that signs requests with SigV4 and retries on transient errors.
pub struct SigV4Client<'a> {
    client: &'a Client,
    endpoint: String,
    region: &'a str,
    creds: &'a Credentials,
}

impl<'a> SigV4Client<'a> {
    pub fn new(client: &'a Client, metadata: &'a HostMetadata, creds: &'a Credentials) -> Self {
        let region = &metadata.imds.region;
        let endpoint = build_endpoint(region);

        Self {
            client,
            endpoint,
            region,
            creds,
        }
    }

    /// Signs and sends a POST request with exponential backoff retry.
    pub fn post(&self, body: &[u8]) -> Result<String> {
        let mut delay = Duration::from_secs(1);
        debug!("POST {}", self.endpoint);

        for attempt in 0..=MAX_RETRIES {
            let resp = self
                .build_signed_request(body)?
                .send()
                .context(error::HttpRequestSnafu)?;

            let status = resp.status().as_u16();
            match status {
                200..=299 => return resp.text().context(error::HttpRequestSnafu),
                429 | 500.. if attempt < MAX_RETRIES => {
                    thread::sleep(delay);
                    delay *= 2;
                    continue;
                }
                _ => {
                    let body_text = resp.text().context(error::HttpRequestSnafu)?;
                    return Err(error::Error::Api {
                        status,
                        body: body_text,
                    });
                }
            }
        }
        Err(error::Error::Api {
            status: 0,
            body: "Retries exhausted".into(),
        })
    }

    fn build_signed_request(&self, body: &[u8]) -> Result<reqwest::blocking::RequestBuilder> {
        let identity = AwsCredentials::new(
            &self.creds.access_key_id,
            &self.creds.secret_access_key,
            Some(self.creds.token.clone()),
            None,
            "imds",
        )
        .into();

        let signing_settings = SigningSettings::default();
        let signing_params = v4::SigningParams::builder()
            .identity(&identity)
            .region(self.region)
            .name(SERVICE)
            .time(SystemTime::now())
            .settings(signing_settings)
            .build()
            .map_err(|e| error::Error::Signing {
                message: e.to_string(),
            })?
            .into();

        let signable_request = SignableRequest::new(
            "POST",
            &self.endpoint,
            std::iter::empty(),
            SignableBody::Bytes(body),
        )
        .map_err(|e| error::Error::Signing {
            message: e.to_string(),
        })?;

        let (instructions, _) = sign(signable_request, &signing_params)
            .map_err(|e| error::Error::Signing {
                message: e.to_string(),
            })?
            .into_parts();

        let mut req = self
            .client
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
            .body(body.to_vec());

        for (name, value) in instructions.headers() {
            req = req.header(name, value);
        }

        Ok(req)
    }
}

fn parent_domain(region: &str) -> &'static str {
    if region.starts_with("cn-") {
        "api.amazonwebservices.com.cn"
    } else {
        "api.aws"
    }
}

fn build_endpoint(region: &str) -> String {
    format!(
        "https://inspector2-telemetry.{}.{}/telemetry",
        region,
        parent_domain(region)
    )
}
