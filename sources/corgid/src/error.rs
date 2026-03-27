use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    #[snafu(display("Failed to read application inventory file"))]
    ReadInventory { source: std::io::Error },

    #[snafu(display("Failed to parse application inventory file"))]
    ParseInventory { source: serde_json::Error },

    #[snafu(display("Failed to serialize SBOM"))]
    SerializeSbom { source: serde_json::Error },

    #[snafu(display("Failed to fetch IMDS credentials"))]
    ImdsCredentials { source: reqwest::Error },

    #[snafu(display("Failed to parse IMDS credentials"))]
    ParseCredentials { source: serde_json::Error },

    #[snafu(display("HTTP request failed"))]
    HttpRequest { source: reqwest::Error },

    #[snafu(display("Inspector API error {status}: {body}"))]
    Api { status: u16, body: String },

    #[snafu(display("Failed to parse API response"))]
    ParseResponse { source: serde_json::Error },

    #[snafu(display("Failed to compress SBOM"))]
    Compress { source: std::io::Error },

    #[snafu(display("No instance ID found in IMDS"))]
    NoInstanceId,

    #[snafu(display("Failed to get Bottlerocket version from /etc/bottlerocket-release"))]
    BottlerocketVersion,

    #[snafu(display("Failed to get system info from uname"))]
    SystemInfo { source: std::io::Error },

    #[snafu(display("Failed to parse output as UTF-8"))]
    Utf8 { source: std::string::FromUtf8Error },

    #[snafu(display("Incomplete uname output"))]
    IncompleteUname,

    #[snafu(display("Missing expected field in API response: {field}"))]
    MissingField { field: String },

    #[snafu(display("Failed to {action}"))]
    Session { action: String, source: Box<Error> },

    #[snafu(display("SigV4 signing failed: {message}: {message}"))]
    Signing { message: String },

    #[snafu(display("Failed to parse instance identity document"))]
    AccountId { source: serde_json::Error },
}

pub type Result<T> = std::result::Result<T, Error>;
