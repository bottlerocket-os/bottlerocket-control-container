//! Host metadata collection from IMDS and system info.

use crate::error::{self, Result};
use crate::imds::ImdsClient;
use log::warn;
use serde::Deserialize;
use snafu::{OptionExt, ResultExt};
use std::process::Command;

const DEFAULT_REGION: &str = "us-east-1";

/// Parsed from the instance identity document.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityDocument {
    pub account_id: String,
}

/// Metadata fetched from IMDS endpoints.
pub struct ImdsMetadata {
    pub region: String,
    pub instance_id: String,
    pub hostname: String,
    pub instance_type: String,
    pub partition: String,
}

impl ImdsMetadata {
    fn new(imds: &ImdsClient) -> Result<Self> {
        let region = imds
            .get("/latest/meta-data/placement/region")
            .unwrap_or_else(|_| {
                warn!("IMDS returned no region, using default: {}", DEFAULT_REGION);
                DEFAULT_REGION.to_string()
            });

        let instance_id = imds
            .get("/latest/meta-data/instance-id")
            .map_err(|_| error::Error::NoInstanceId)?;

        Ok(Self {
            region,
            instance_id,
            hostname: imds.get("/latest/meta-data/hostname")?,
            instance_type: imds.get("/latest/meta-data/instance-type")?,
            partition: imds.get("/latest/meta-data/services/partition")?,
        })
    }
}

/// System information from uname.
pub struct SystemInfo {
    pub kernel_name: String,
    pub kernel_version: String,
    pub cpu_architecture: String,
}

impl SystemInfo {
    fn new() -> Result<Self> {
        let output = Command::new("uname")
            .arg("-srm")
            .output()
            .context(error::SystemInfoSnafu)?;

        let output = String::from_utf8(output.stdout).context(error::Utf8Snafu)?;
        let mut parts = output.trim().splitn(3, ' ');
        Ok(Self {
            kernel_name: parts
                .next()
                .context(error::IncompleteUnameSnafu)?
                .to_string(),
            kernel_version: parts
                .next()
                .context(error::IncompleteUnameSnafu)?
                .to_string(),
            cpu_architecture: parts
                .next()
                .context(error::IncompleteUnameSnafu)?
                .to_string(),
        })
    }
}

/// Composed host metadata from IMDS, identity document, and system info.
pub struct HostMetadata {
    pub imds: ImdsMetadata,
    pub identity: IdentityDocument,
    pub system: SystemInfo,
}

impl HostMetadata {
    pub fn new() -> Result<Self> {
        let imds = ImdsClient::new()?;

        let imds_metadata = ImdsMetadata::new(&imds)?;
        let identity = Self::fetch_identity(&imds)?;
        let system = SystemInfo::new()?;

        Ok(Self {
            imds: imds_metadata,
            identity,
            system,
        })
    }

    fn fetch_identity(imds: &ImdsClient) -> Result<IdentityDocument> {
        let doc = imds.get("/latest/dynamic/instance-identity/document")?;
        serde_json::from_str(&doc).context(error::AccountIdSnafu)
    }
}
