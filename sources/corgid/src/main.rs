//! corgid - Bottlerocket package inventory reporter for Amazon Inspector
//!
//! Reads the disable flag from user-data before doing any work.

#![deny(unused_imports)]
#![deny(missing_docs)]

mod error;
mod imds;
mod inspector;
mod inventory;
mod metadata;
mod sbom;
mod sigv4;

use error::Result;
use imds::Credentials;
use inspector::InspectorClient;
use inventory::Inventory;
use log::{debug, info};
use metadata::HostMetadata;
use reqwest::blocking::Client;
use sbom::Sbom;
use serde::Deserialize;
use simplelog::{Config as LogConfig, LevelFilter, SimpleLogger};

/// Path to the control container user-data file.
const USER_DATA_PATH: &str = "/.bottlerocket/host-containers/current/user-data";
/// Path to the Bottlerocket application inventory on the host.
const INVENTORY_PATH: &str = "/var/lib/bottlerocket/inventory/application.json";

#[snafu::report]
fn main() -> Result<()> {
    let log_level = std::env::var("CORGID_LOG_LEVEL")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(LevelFilter::Info);
    SimpleLogger::init(log_level, LogConfig::default()).expect("logger init");

    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install crypto provider");

    // Check user-data disable flag before doing any work
    let config = CorgidConfig::from_path(USER_DATA_PATH);
    if !config.inspector_enabled() {
        info!("Inspector SBOM upload disabled via user-data, exiting");
        return Ok(());
    }

    info!("Starting Inspector SBOM upload");

    debug!("Fetching metadata");
    let metadata = HostMetadata::new()?;

    debug!("Reading inventory");
    let inventory = Inventory::from_path(INVENTORY_PATH)?;

    debug!("Building SBOM");
    let os_release = sbom::OsRelease::from_file()?;
    let sbom = Sbom::new(&metadata, &inventory, &os_release)?;
    let sbom_json = sbom.to_json()?;
    let _ = std::fs::write("/tmp/corgid-sbom.json", &sbom_json);
    let sbom_hash = sbom.digest()?;

    debug!("Fetching IAM credentials");
    let creds = Credentials::from_imds()?;

    let client = Client::new();
    let sigv4 = sigv4::SigV4Client::new(&client, &metadata, &creds);
    let inspector = InspectorClient::new(sigv4, &metadata);

    debug!("Starting session");
    let session = inspector.start_session()?;
    info!("Inspector session started");

    debug!("Sending SBOM");
    match session.send_sbom(&sbom_json) {
        Ok(()) => {
            session.close(&sbom_hash, inspector::SessionStatus::Successful)?;
            info!("Inspector SBOM upload completed successfully");
            Ok(())
        }
        Err(e) => {
            session.close(&sbom_hash, inspector::SessionStatus::Failure)?;
            Err(e)
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", default)]
struct CorgidConfig {
    upload_sbom: bool,
}

impl Default for CorgidConfig {
    fn default() -> Self {
        Self { upload_sbom: true }
    }
}

#[derive(Deserialize)]
struct UserData {
    #[serde(default)]
    inspector: Option<CorgidConfig>,
}

impl CorgidConfig {
    fn from_path(path: &str) -> Self {
        // If user-data file is absent or unreadable, default to enabled
        let Ok(data) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        // If user-data is not valid JSON, default to enabled
        let Ok(user_data) = serde_json::from_str::<UserData>(&data) else {
            return Self::default();
        };
        user_data.inspector.unwrap_or_default()
    }

    fn inspector_enabled(&self) -> bool {
        self.upload_sbom
    }
}
