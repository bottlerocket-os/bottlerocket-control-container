//! CycloneDX SBOM generation.

use crate::error::{self, Result};
use crate::inventory::{Inventory, Package};
use crate::metadata::HostMetadata;
use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use sha2::Digest;
use snafu::{OptionExt, ResultExt};
use uuid::Uuid;

const INVENTORY_VERSION: &str = "0.1.0";
const BR_RELEASE_PATH: &str = "/etc/bottlerocket-release";

/// SBOM metadata including generation timestamp and tooling info.
#[derive(Serialize)]
pub struct Metadata {
    pub timestamp: String,
    pub tools: Tools,
}

/// Container for tool components that generated this SBOM.
#[derive(Serialize)]
pub struct Tools {
    pub components: Vec<ToolComponent>,
}

/// Describes the tool used to generate the SBOM.
#[derive(Serialize)]
pub struct ToolComponent {
    #[serde(rename = "type")]
    pub component_type: String,
    pub author: String,
    pub name: String,
    pub version: String,
}

/// Wrapper for license information in CycloneDX format.
#[derive(Serialize)]
pub struct LicenseEntry {
    pub license: LicenseId,
}

/// SPDX license identifier.
#[derive(Serialize)]
pub struct LicenseId {
    pub id: String,
}

/// A key-value property using the `amazon:inspector:sbom_generator:metadata:` prefix
/// to conform to Amazon Inspector's expected schema for host/IMDS data.
#[derive(Serialize)]
pub struct Property {
    pub name: String,
    pub value: String,
}

impl From<&HostMetadata> for Vec<Property> {
    fn from(m: &HostMetadata) -> Self {
        vec![
            Property {
                name: "amazon:inspector:sbom_generator:metadata:host:hostname".into(),
                value: m.imds.hostname.clone(),
            },
            Property {
                name: "amazon:inspector:sbom_generator:metadata:host:kernel_name".into(),
                value: m.system.kernel_name.clone(),
            },
            Property {
                name: "amazon:inspector:sbom_generator:metadata:host:kernel_version".into(),
                value: m.system.kernel_version.clone(),
            },
            Property {
                name: "amazon:inspector:sbom_generator:metadata:host:cpu_architecture".into(),
                value: m.system.cpu_architecture.clone(),
            },
            Property {
                name: "amazon:inspector:sbom_generator:metadata:imds:provider".into(),
                value: "aws".into(),
            },
            Property {
                name: "amazon:inspector:sbom_generator:metadata:imds:instance_id".into(),
                value: m.imds.instance_id.clone(),
            },
            Property {
                name: "amazon:inspector:sbom_generator:metadata:imds:instance_type".into(),
                value: m.imds.instance_type.clone(),
            },
            Property {
                name: "amazon:inspector:sbom_generator:metadata:imds:instance_location".into(),
                value: m.imds.region.clone(),
            },
            Property {
                name: "amazon:inspector:sbom_generator:metadata:imds:instance_partition".into(),
                value: m.imds.partition.clone(),
            },
            Property {
                name: "amazon:inspector:sbom_generator:metadata:imds:account_id".into(),
                value: m.identity.account_id.clone(),
            },
            Property {
                name: "amazon:inspector:sbom_generator:metadata:imds:resource_type".into(),
                value: "ec2:instance".into(),
            },
        ]
    }
}

/// Parsed fields from /etc/bottlerocket-release.
pub struct OsRelease {
    pub name: String,
    pub version_id: String,
    pub pretty_name: String,
}

impl OsRelease {
    pub fn from_file() -> Result<Self> {
        let content =
            std::fs::read_to_string(BR_RELEASE_PATH).context(error::ReadInventorySnafu)?;
        Self::parse(&content)
    }

    fn parse(content: &str) -> Result<Self> {
        let get = |key: &str| -> Option<String> {
            content
                .lines()
                .find(|l| l.starts_with(key))
                .map(|l| l[key.len()..].trim_matches('"').to_string())
        };
        Ok(Self {
            name: get("NAME=").context(error::BottlerocketVersionSnafu)?,
            version_id: get("VERSION_ID=").context(error::BottlerocketVersionSnafu)?,
            pretty_name: get("PRETTY_NAME=").context(error::BottlerocketVersionSnafu)?,
        })
    }
}

/// A software component in the SBOM.
#[derive(Serialize)]
pub struct Component {
    #[serde(rename = "bom-ref")]
    pub bom_ref: String,
    #[serde(rename = "type")]
    pub component_type: String,
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purl: Option<String>,
    pub publisher: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub licenses: Option<Vec<LicenseEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Vec<Property>>,
}

impl Component {
    fn from_package(pkg: &Package, index: usize, version_id: &str) -> Self {
        let epoch = if pkg.epoch.is_empty() {
            "0"
        } else {
            &pkg.epoch
        };
        Self {
            bom_ref: format!("comp-{}", index),
            component_type: "library".into(),
            name: pkg.name.clone(),
            version: format!("{}-{}", pkg.version, pkg.release),
            purl: Some(format!(
                "pkg:rpm/bottlerocket/{}@{}-{}?arch={}&epoch={}&distro={}",
                pkg.name, pkg.version, pkg.release, pkg.architecture, epoch, version_id,
            )),
            publisher: pkg.publisher.clone(),
            description: pkg.summary.clone(),
            licenses: None,
            properties: None,
        }
    }
}

/// Root CycloneDX SBOM document structure.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sbom {
    pub bom_format: String,
    pub spec_version: String,
    pub serial_number: String,
    pub version: u32,
    pub metadata: Metadata,
    pub components: Vec<Component>,
}

impl Sbom {
    pub fn new(
        metadata: &HostMetadata,
        inventory: &Inventory,
        os_release: &OsRelease,
    ) -> Result<Self> {
        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
        let properties: Vec<Property> = metadata.into();
        let os_component = Component {
            bom_ref: "comp-os".into(),
            component_type: "operating-system".into(),
            name: os_release.name.clone(),
            version: os_release.version_id.clone(),
            purl: None,
            publisher: "Amazon Web Services, Inc. (AWS)".into(),
            description: os_release.pretty_name.clone(),
            licenses: None,
            properties: Some(properties),
        };

        let mut components = vec![os_component];
        components.extend(
            inventory
                .content
                .iter()
                .enumerate()
                .map(|(i, p)| Component::from_package(p, i, &os_release.version_id)),
        );

        let tool_component = ToolComponent {
            component_type: "application".into(),
            author: "Amazon Web Services, Inc. (AWS)".into(),
            name: "corgid".into(),
            version: INVENTORY_VERSION.into(),
        };

        Ok(Self {
            bom_format: "CycloneDX".into(),
            spec_version: "1.5".into(),
            serial_number: format!("urn:uuid:{}", Uuid::new_v4()),
            version: 1,
            metadata: Metadata {
                timestamp,
                tools: Tools {
                    components: vec![tool_component],
                },
            },
            components,
        })
    }

    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).context(error::SerializeSbomSnafu)
    }

    pub fn digest(&self) -> Result<String> {
        let json = self.to_json()?;
        Ok(hex::encode(sha2::Sha256::digest(json.as_bytes())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::Package;

    #[test]
    fn test_component_from_package() {
        let pkg = Package {
            name: "glibc".into(),
            publisher: "bottlerocket-core-kit".into(),
            version: "2.38".into(),
            release: "1.br1".into(),
            epoch: "0".into(),
            architecture: "x86_64".into(),
            _url: "".into(),
            summary: "GNU C Library".into(),
        };

        let component = Component::from_package(&pkg, 0, "1.47.0");

        assert_eq!(component.component_type, "library");
        assert_eq!(component.name, "glibc");
        assert_eq!(component.version, "2.38-1.br1");
        assert_eq!(component.bom_ref, "comp-0");
        assert_eq!(
            component.purl.as_ref().unwrap(),
            "pkg:rpm/bottlerocket/glibc@2.38-1.br1?arch=x86_64&epoch=0&distro=1.47.0"
        );
    }

    #[test]
    fn test_component_from_package_empty_epoch() {
        let pkg = Package {
            name: "test-pkg".into(),
            publisher: "test".into(),
            version: "1.0".into(),
            release: "1".into(),
            epoch: "".into(),
            architecture: "aarch64".into(),
            _url: "".into(),
            summary: "Test".into(),
        };

        let component = Component::from_package(&pkg, 5, "1.47.0");
        assert!(component.purl.as_ref().unwrap().contains("&epoch=0&"));
    }

    #[test]
    fn test_sbom_new() {
        let metadata = crate::metadata::HostMetadata {
            imds: crate::metadata::ImdsMetadata {
                region: "us-west-2".into(),
                instance_id: "i-0123456789abcdef0".into(),
                hostname: "test-host".into(),
                instance_type: "m5.large".into(),
                partition: "aws".into(),
            },
            identity: crate::metadata::IdentityDocument {
                account_id: "123456789012".into(),
            },
            system: crate::metadata::SystemInfo {
                kernel_name: "Linux".into(),
                kernel_version: "6.1.100".into(),
                cpu_architecture: "x86_64".into(),
            },
        };
        let inventory = crate::inventory::Inventory {
            content: vec![Package {
                name: "glibc".into(),
                publisher: "bottlerocket-core-kit".into(),
                version: "2.38".into(),
                release: "1.br1".into(),
                epoch: "0".into(),
                architecture: "x86_64".into(),
                _url: "".into(),
                summary: "GNU C Library".into(),
            }],
        };
        let os_release = OsRelease {
            name: "Bottlerocket".into(),
            version_id: "1.47.0".into(),
            pretty_name: "Bottlerocket OS 1.47.0".into(),
        };

        let sbom = Sbom::new(&metadata, &inventory, &os_release).unwrap();

        // Top-level fields
        assert_eq!(sbom.bom_format, "CycloneDX");
        assert_eq!(sbom.spec_version, "1.5");
        assert_eq!(sbom.version, 1);
        assert_eq!(sbom.metadata.tools.components[0].name, "corgid");

        // OS component
        assert_eq!(sbom.components[0].component_type, "operating-system");
        assert_eq!(sbom.components[0].name, "Bottlerocket");
        assert_eq!(sbom.components[0].version, "1.47.0");
        assert!(sbom.components[0].purl.is_none());
        let props = sbom.components[0].properties.as_ref().unwrap();
        let instance_id_prop = props
            .iter()
            .find(|p| p.name.contains("instance_id"))
            .unwrap();
        assert_eq!(instance_id_prop.value, "i-0123456789abcdef0");

        // Library component
        assert_eq!(sbom.components[1].component_type, "library");
        assert_eq!(sbom.components[1].name, "glibc");
        assert_eq!(sbom.components[1].version, "2.38-1.br1");
        assert_eq!(
            sbom.components[1].purl.as_ref().unwrap(),
            "pkg:rpm/bottlerocket/glibc@2.38-1.br1?arch=x86_64&epoch=0&distro=1.47.0"
        );
        assert_eq!(sbom.components[1].publisher, "bottlerocket-core-kit");
        assert_eq!(sbom.components[1].description, "GNU C Library");

        // Total components: 1 OS + 1 library
        assert_eq!(sbom.components.len(), 2);
    }

    #[test]
    fn test_os_release_parse() {
        let content = r#"NAME=Bottlerocket
ID=bottlerocket
VERSION="1.47.0 (aws-k8s-1.34)"
PRETTY_NAME="Bottlerocket OS 1.47.0 (aws-k8s-1.34)"
VARIANT_ID=aws-k8s-1.34
VERSION_ID=1.47.0
BUILD_ID=6154605b
VENDOR_NAME=Bottlerocket
"#;
        let os_release = OsRelease::parse(content).unwrap();
        assert_eq!(os_release.name, "Bottlerocket");
        assert_eq!(os_release.version_id, "1.47.0");
        assert_eq!(
            os_release.pretty_name,
            "Bottlerocket OS 1.47.0 (aws-k8s-1.34)"
        );
    }

    #[test]
    fn test_os_release_parse_missing_field() {
        let content = "NAME=Bottlerocket\nVERSION_ID=1.47.0\n";
        let result = OsRelease::parse(content);
        assert!(result.is_err());
    }
}
