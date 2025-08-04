use std::collections::HashMap;
use std::io::{self, Write};

#[derive(Debug, Clone, serde::Serialize)]
pub struct ErrorReport {
    pub message: String,
    pub kind: String,
    pub data: HashMap<String, String>,
    pub help: Option<String>,
    pub code: Option<u32>,
}

impl ErrorReport {
    pub fn new(message: String, kind: String) -> Self {
        Self {
            message,
            kind,
            data: HashMap::new(),
            help: None,
            code: None,
        }
    }

    pub fn with_data(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.data.insert(key.into(), value.into());
        self
    }

    pub fn with_help(mut self, help: String) -> Self {
        self.help = Some(help);
        self
    }

    pub fn with_code(mut self, code: u32) -> Self {
        self.code = Some(code);
        self
    }

    /// Print the error report to stderr with structured formatting
    pub fn print(&self) {
        let mut stderr = io::stderr();

        // Print error header
        let _ = writeln!(stderr, "❗️ error: {}", self.message);

        // Print additional data if available
        if !self.data.is_empty() {
            let _ = writeln!(stderr, "   details:");
            for (key, value) in &self.data {
                let _ = writeln!(stderr, "     {}: {}", key, value);
            }
        }

        // Print help message if available
        if let Some(help) = &self.help {
            let _ = writeln!(stderr, "   💡 {}", help);
        }

        let _ = writeln!(stderr);
    }

    /// Print the error report to stdout with JSON formatting
    pub fn print_json(&self) {
        let json = serde_json::to_string_pretty(self)
            .unwrap_or_else(|_| format!("{{\"error\": \"Failed to serialize error report\"}}"));
        println!("{}", json);
    }
}

impl std::fmt::Display for ErrorReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error: {} (Type: {})", self.message, self.kind)?;

        if let Some(code) = self.code {
            write!(f, " [Code: {}]", code)?;
        }

        if !self.data.is_empty() {
            write!(f, " - Details: {:?}", self.data)?;
        }

        if let Some(help) = &self.help {
            write!(f, " - Help: {}", help)?;
        }

        Ok(())
    }
}

// From trait implementations for different error types

impl From<tx3_sdk::trp::Error> for ErrorReport {
    fn from(error: tx3_sdk::trp::Error) -> Self {
        match error {
            tx3_sdk::trp::Error::NetworkError(network_error) => ErrorReport::new(
                "Network communication error".to_string(),
                "network".to_string(),
            )
            .with_code(1)
            .with_data("error".to_string(), network_error.to_string()),
            tx3_sdk::trp::Error::HttpError(status, message) => {
                ErrorReport::new("HTTP request failed".to_string(), "http".to_string())
                    .with_code(2)
                    .with_data("status", status.to_string())
                    .with_data("message", message)
            }
            tx3_sdk::trp::Error::DeserializationError(deserialization_error) => ErrorReport::new(
                "Failed to deserialize response".to_string(),
                "deserialization".to_string(),
            )
            .with_code(3)
            .with_data("error".to_string(), deserialization_error.to_string()),
            tx3_sdk::trp::Error::GenericRpcError(method, params, value) => {
                ErrorReport::new("Generic RPC error".to_string(), "rpc".to_string())
                    .with_data("method".to_string(), method.to_string())
                    .with_data("parameters".to_string(), format!("{:?}", params))
                    .with_data("value".to_string(), format!("{:?}", value))
            }
            tx3_sdk::trp::Error::UnknownError(message) => {
                ErrorReport::new("Unknown error occurred".to_string(), "unknown".to_string())
                    .with_data("message".to_string(), message)
            }
            tx3_sdk::trp::Error::UnsupportedTir(x) => ErrorReport::new(
                "Unsupported TIR version or feature".to_string(),
                "tir".to_string(),
            )
            .with_data("expected", x.expected)
            .with_data("provided", x.provided),
            tx3_sdk::trp::Error::InvalidTirEnvelope => {
                ErrorReport::new("Invalid TIR envelope".to_string(), "tir".to_string())
            }
            tx3_sdk::trp::Error::InvalidTirBytes => {
                ErrorReport::new("Invalid TIR bytes".to_string(), "tir".to_string())
            }
            tx3_sdk::trp::Error::UnsupportedTxEra => {
                ErrorReport::new("Unsupported transaction era".to_string(), "era".to_string())
            }
            tx3_sdk::trp::Error::UnsupportedEra { era } => {
                ErrorReport::new("Unsupported era".to_string(), "era".to_string())
                    .with_data("era", era.to_string())
            }
            tx3_sdk::trp::Error::MissingTxArg(x) => ErrorReport::new(
                "Missing transaction argument".to_string(),
                "args".to_string(),
            )
            .with_data("arg", x.key)
            .with_data("type", x.ty),
            tx3_sdk::trp::Error::InputNotResolved(x) => {
                ErrorReport::new("Input not resolved".to_string(), "input".to_string())
                    .with_data("input", x.name)
                    .with_data("query.address", format!("{:?}", x.query.address))
                    .with_data("query.min_amount", format!("{:?}", x.query.min_amount))
                    .with_data("query.refs", format!("{:?}", x.query.refs))
                    .with_data("query.collateral", format!("{}", x.query.collateral))
                    .with_data("query.support_many", format!("{}", x.query.support_many))
            }
            tx3_sdk::trp::Error::TxScriptFailure(tx_script_failure) => ErrorReport::new(
                "Transaction script execution failed".to_string(),
                "script".to_string(),
            )
            .with_data("script_failure".to_string(), tx_script_failure.to_string()),
        }
    }
}

impl From<utxorpc::Error> for ErrorReport {
    fn from(error: utxorpc::Error) -> Self {
        match error {
            utxorpc::Error::TransportError(transport_error) => {
                ErrorReport::new("Transport error".to_string(), "transport".to_string())
                    .with_code(20)
                    .with_data("error".to_string(), transport_error.to_string())
            }
            utxorpc::Error::GrpcError(status) => {
                ErrorReport::new("gRPC error".to_string(), "grpc".to_string())
                    .with_code(21)
                    .with_data("status_message".to_string(), status.message().to_string())
                    .with_data("status_code".to_string(), format!("{}", status.code()))
            }
            utxorpc::Error::ParseError(parse_error) => {
                ErrorReport::new("Parse error".to_string(), "parse".to_string())
                    .with_code(22)
                    .with_data("error".to_string(), parse_error.to_string())
            }
        }
    }
}

impl From<String> for ErrorReport {
    fn from(error_msg: String) -> Self {
        ErrorReport::new(error_msg, "generic".to_string())
    }
}

impl From<anyhow::Error> for ErrorReport {
    fn from(error: anyhow::Error) -> Self {
        // Try to downcast to specific error types first
        if error.downcast_ref::<tx3_sdk::trp::Error>().is_some() {
            let error = error.downcast::<tx3_sdk::trp::Error>().unwrap();
            return ErrorReport::from(error);
        }

        if error.downcast_ref::<utxorpc::Error>().is_some() {
            let error = error.downcast::<utxorpc::Error>().unwrap();
            return ErrorReport::from(error);
        }

        // Generic error report
        ErrorReport::from(error.to_string())
    }
}
