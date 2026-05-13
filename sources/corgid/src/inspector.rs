//! Amazon Inspector API client.
//!
//! Manages the session lifecycle: start -> send SBOM -> stop.

use crate::error::{self, Result};
use crate::metadata::HostMetadata;
use crate::sigv4::SigV4Client;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use snafu::{OptionExt, ResultExt};
use std::io::Write;

/// Version string sent to Inspector in the start session request.
const AGENT_VERSION: &str = "0.1.0";

/// SBOM chunks are limited to 390KB to stay within API payload limits.
/// Maximum size of each compressed SBOM chunk sent to Inspector.
const CHUNK_SIZE: usize = 390 * 1024;

/// Initiates a new vulnerability scan session.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StartSessionEvent<'a> {
    session_scan_type: &'a str,
    resource_type: &'a str,
    agent_version: &'a str,
}

/// SBOM chunk with integrity hash for verification.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VulnData<'a> {
    content_hash: &'a str,
    #[serde(serialize_with = "serialize_base64")]
    sbom: &'a [u8],
    sequence_number: u32,
}

/// Payload data containing vulnerability scan results.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TelemetryData<'a> {
    vulnerability_data: VulnData<'a>,
}

/// Sends SBOM data chunk within an active session.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SendTelemetryEvent<'a> {
    session_id: &'a str,
    capture_time: f64,
    data: TelemetryData<'a>,
}

/// Session outcome details sent when closing a session.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionDetails<'a> {
    session_status: &'a str,
}

/// Scan job details including status and performance metrics.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanDetails<'a> {
    scan_job_status: &'a str,
    data_checksum: &'a str,
    performance_details: PerformanceDetails,
}

/// Terminates a scan session with final status and metrics.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StopSessionEvent<'a> {
    session_id: &'a str,
    session_details: SessionDetails<'a>,
    scan_details: ScanDetails<'a>,
}

/// Discriminated union of event types.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum TelemetryEvent<'a> {
    StartSession(StartSessionEvent<'a>),
    SendTelemetry(SendTelemetryEvent<'a>),
    StopSession(StopSessionEvent<'a>),
}

/// Wrapper for all API requests.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TelemetryRequest<'a> {
    resource_id: &'a str,
    event: TelemetryEvent<'a>,
}

/// CPU usage metrics.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct CpuMetrics {
    average_cpu: f64,
    max_cpu: f64,
}

/// Memory usage metrics.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct MemoryMetrics {
    average_memory_consumed: f64,
    max_memory_consumed: f64,
}

/// Artifact collection metrics.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct ArtifactMetrics {
    data_collection_in_milliseconds: i64,
}

/// Resource usage metrics.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct PerformanceDetails {
    cpu_metrics: CpuMetrics,
    memory_metrics: MemoryMetrics,
    artifact_metrics: ArtifactMetrics,
}

/// Response from the start session API call.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartSessionResult {
    session_id: String,
}

/// API response containing optional session start result.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TelemetryResponse {
    start_session_result: Option<StartSessionResult>,
}

/// Session outcome reported to Inspector.
#[derive(Clone, Copy)]
pub enum SessionStatus {
    /// Session completed successfully.
    Successful,
    /// Session failed.
    Failure,
}

impl From<SessionStatus> for &'static str {
    fn from(status: SessionStatus) -> Self {
        match status {
            SessionStatus::Successful => "SUCCESSFUL",
            SessionStatus::Failure => "FAILURE",
        }
    }
}

/// Scan job status reported to Inspector.
#[derive(Clone, Copy)]
pub enum ScanJobStatus {
    /// Scan completed successfully.
    Completed,
    /// Scan failed due to an internal error.
    AgentInternalError,
}

impl From<ScanJobStatus> for &'static str {
    fn from(status: ScanJobStatus) -> Self {
        match status {
            ScanJobStatus::Completed => "COMPLETED",
            ScanJobStatus::AgentInternalError => "AGENT_INTERNAL_ERROR",
        }
    }
}

impl From<SessionStatus> for ScanJobStatus {
    fn from(status: SessionStatus) -> Self {
        match status {
            SessionStatus::Successful => Self::Completed,
            SessionStatus::Failure => Self::AgentInternalError,
        }
    }
}

/// Client for the Inspector telemetry API.
pub struct InspectorClient<'a> {
    sigv4_client: SigV4Client<'a>,
    instance_id: &'a str,
}

impl<'a> InspectorClient<'a> {
    pub fn new(sigv4_client: SigV4Client<'a>, metadata: &'a HostMetadata) -> Self {
        Self {
            sigv4_client,
            instance_id: &metadata.imds.instance_id,
        }
    }

    /// Starts a session, consuming the client.
    pub fn start_session(self) -> Result<OpenSession<'a>> {
        let start_event = StartSessionEvent {
            session_scan_type: "VULNERABILITY_SCAN",
            resource_type: "AWS_EC2_INSTANCE",
            agent_version: AGENT_VERSION,
        };
        let req = TelemetryRequest {
            resource_id: self.instance_id,
            event: TelemetryEvent::StartSession(start_event),
        };
        let body = serde_json::to_vec(&req).context(error::SerializeSbomSnafu)?;
        let resp = self.sigv4_client.post(&body)?;

        let parsed: TelemetryResponse =
            serde_json::from_str(&resp).context(error::ParseResponseSnafu)?;
        let result = parsed
            .start_session_result
            .context(error::MissingFieldSnafu {
                field: "startSessionResult",
            })?;
        Ok(OpenSession {
            sigv4_client: self.sigv4_client,
            instance_id: self.instance_id,
            session_id: result.session_id,
        })
    }
}

/// An active Inspector session.
pub struct OpenSession<'a> {
    sigv4_client: SigV4Client<'a>,
    instance_id: &'a str,
    session_id: String,
}

impl<'a> OpenSession<'a> {
    /// Sends SBOM data, compressed and chunked.
    pub fn send_sbom(&self, sbom: &str) -> Result<()> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(sbom.as_bytes())
            .context(error::CompressSnafu)?;
        let compressed = encoder.finish().context(error::CompressSnafu)?;

        let capture_time = Utc::now().timestamp() as f64;

        for (seq, chunk) in compressed.chunks(CHUNK_SIZE).enumerate() {
            let chunk_hash = hex::encode(Sha256::digest(chunk));
            let vuln_data = VulnData {
                content_hash: &chunk_hash,
                sbom: chunk,
                sequence_number: seq as u32,
            };
            let telemetry_data = TelemetryData {
                vulnerability_data: vuln_data,
            };
            let send_event = SendTelemetryEvent {
                session_id: &self.session_id,
                capture_time,
                data: telemetry_data,
            };
            let req = TelemetryRequest {
                resource_id: self.instance_id,
                event: TelemetryEvent::SendTelemetry(send_event),
            };
            let body = serde_json::to_vec(&req).context(error::SerializeSbomSnafu)?;
            self.sigv4_client.post(&body)?;
        }
        Ok(())
    }

    /// Closes the session, consuming the OpenSession.
    pub fn close(self, sbom_hash: &str, status: SessionStatus) -> Result<()> {
        let session_status: &str = status.into();
        let scan_job_status: &str = ScanJobStatus::from(status).into();
        let scan_details = ScanDetails {
            scan_job_status,
            data_checksum: sbom_hash,
            performance_details: PerformanceDetails::default(),
        };
        let req = TelemetryRequest {
            resource_id: self.instance_id,
            event: TelemetryEvent::StopSession(StopSessionEvent {
                session_id: &self.session_id,
                session_details: SessionDetails { session_status },
                scan_details,
            }),
        };
        let body = serde_json::to_vec(&req).context(error::SerializeSbomSnafu)?;
        self.sigv4_client.post(&body)?;

        Ok(())
    }
}

/// Serializes byte slices as base64 strings for JSON encoding.
fn serialize_base64<S: serde::Serializer>(
    data: &&[u8],
    serializer: S,
) -> std::result::Result<S::Ok, S::Error> {
    serializer.serialize_str(&BASE64.encode(data))
}
