//! Bottlerocket application inventory.

use crate::error::{self, Result};
use serde::Deserialize;
use snafu::ResultExt;
use std::fs;

/// Deserialized application inventory from Bottlerocket's package list.
#[derive(Deserialize)]
pub struct Inventory {
    #[serde(rename = "Content")]
    pub content: Vec<Package>,
}

/// Individual package entry from the inventory file.
#[derive(Deserialize)]
pub struct Package {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Publisher")]
    pub publisher: String,
    #[serde(rename = "Version")]
    pub version: String,
    #[serde(rename = "Release")]
    pub release: String,
    #[serde(rename = "Epoch")]
    pub epoch: String,
    #[serde(rename = "Architecture")]
    pub architecture: String,
    #[serde(rename = "Url")]
    pub _url: String,
    #[serde(rename = "Summary")]
    pub summary: String,
}

impl Inventory {
    pub fn from_path(path: &str) -> Result<Self> {
        let data = fs::read_to_string(path).context(error::ReadInventorySnafu)?;
        serde_json::from_str(&data).context(error::ParseInventorySnafu)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_path_success() {
        let path = "/tmp/test-inventory-success.json";
        std::fs::write(path, r#"{"Content":[{"Name":"test","Publisher":"pub","Version":"1.0","Release":"1","Epoch":"0","Architecture":"x86_64","Url":"","Summary":"desc"}]}"#).unwrap();

        let inv = Inventory::from_path(path).unwrap();
        assert_eq!(inv.content.len(), 1);
        assert_eq!(inv.content[0].name, "test");
        assert_eq!(inv.content[0].version, "1.0");
        assert_eq!(inv.content[0].release, "1");
        assert_eq!(inv.content[0].architecture, "x86_64");

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn test_from_path_file_not_found() {
        let result = Inventory::from_path("/nonexistent/path");
        assert!(result.is_err());
    }

    #[test]
    fn test_from_path_invalid_json() {
        let path = "/tmp/test-inventory-invalid.json";
        std::fs::write(path, "not json").unwrap();

        let result = Inventory::from_path(path);
        assert!(result.is_err());

        std::fs::remove_file(path).ok();
    }
}
