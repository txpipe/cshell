use std::collections::HashMap;
use std::io::{self, Write};

#[derive(Debug, Clone, serde::Serialize)]
pub struct ErrorReport {
    pub message: String,
    pub kind: String,
    pub details: HashMap<String, String>,
    pub logs: Vec<String>,
    pub help: Option<String>,
    pub code: Option<u32>,
}

impl ErrorReport {
    pub fn new(message: String, kind: String) -> Self {
        Self {
            message,
            kind,
            details: HashMap::new(),
            logs: vec![],
            help: None,
            code: None,
        }
    }

    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn with_code(mut self, code: u32) -> Self {
        self.code = Some(code);
        self
    }

    pub fn with_logs(mut self, logs: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.logs = logs.into_iter().map(|x| x.into()).collect();
        self
    }

    /// Print the error report to stderr with structured formatting
    pub fn print(&self) {
        let mut stderr = io::stderr();

        // Print error header
        let _ = writeln!(stderr, "❗️ error: {}", self.message);

        // Print additional data if available
        if !self.details.is_empty() {
            let _ = writeln!(stderr, "   details:");
            for (key, value) in &self.details {
                let _ = writeln!(stderr, "   ∙ {key}: {value}");
            }
        }

        // Print help message if available
        if let Some(help) = &self.help {
            let _ = writeln!(stderr, "💡 {help}");
        }

        if !self.logs.is_empty() {
            let _ = writeln!(stderr, "   logs:");
            for log in &self.logs {
                let _ = writeln!(stderr, "   ‣ {log}");
            }
        }

        let _ = writeln!(stderr);
    }
}

impl std::fmt::Display for ErrorReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error: {} (Type: {})", self.message, self.kind)?;

        if let Some(code) = self.code {
            write!(f, " [Code: {code}]")?;
        }

        if !self.details.is_empty() {
            write!(f, " - Details: {:?}", self.details)?;
        }

        if let Some(help) = &self.help {
            write!(f, " - Help: {help}")?;
        }

        Ok(())
    }
}

// From trait implementations for different error types

impl From<tx3_sdk::trp::Error> for ErrorReport {
    fn from(error: tx3_sdk::trp::Error) -> Self {
        match error {
            tx3_sdk::trp::Error::NetworkError(err) => {
                ErrorReport::new(err.to_string(), "network".to_string())
            }
            tx3_sdk::trp::Error::HttpError(status, message) => {
                ErrorReport::new(message, "http".to_string())
                    .with_code(2)
                    .with_detail("status", status.to_string())
            }
            tx3_sdk::trp::Error::DeserializationError(message) => {
                ErrorReport::new(message, "deserialization".to_string())
            }
            tx3_sdk::trp::Error::GenericRpcError(code, message, value) => {
                ErrorReport::new(message, "trp".to_string())
                    .with_detail("code".to_string(), code.to_string())
                    .with_detail(
                        "data".to_string(),
                        serde_json::to_string(&value).unwrap_or_default(),
                    )
            }
            tx3_sdk::trp::Error::UnknownError(message) => {
                ErrorReport::new("Unknown error occurred".to_string(), "unknown".to_string())
                    .with_detail("message".to_string(), message)
            }
            tx3_sdk::trp::Error::UnsupportedTir(x) => ErrorReport::new(
                "The TIR version is not supported by the server".to_string(),
                "trp".to_string(),
            )
            .with_detail("expected", x.expected)
            .with_detail("provided", x.provided)
            .with_help(
                "Make sure that the Tx3 version on your machine is compatible with the TRP server.",
            ),
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
                    .with_detail("era", era.to_string())
            }
            tx3_sdk::trp::Error::MissingTxArg(x) => ErrorReport::new(
                "Missing transaction argument".to_string(),
                "args".to_string(),
            )
            .with_detail("arg", x.key)
            .with_detail("type", x.ty),
            tx3_sdk::trp::Error::InputNotResolved(x) => {
                ErrorReport::new("Input not resolved".to_string(), "input".to_string())
                    .with_detail("input", x.name)
                    .with_detail("query.address", format!("{:?}", x.query.address))
                    .with_detail("query.min_amount", format!("{:?}", x.query.min_amount))
                    .with_detail("query.refs", format!("{:?}", x.query.refs))
                    .with_detail("query.collateral", format!("{}", x.query.collateral))
                    .with_detail("query.support_many", format!("{}", x.query.support_many))
            }
            tx3_sdk::trp::Error::TxScriptFailure(x) => ErrorReport::new(
                "Transaction script execution failed".to_string(),
                "script".to_string(),
            )
            .with_logs(x.logs),
        }
    }
}

impl From<utxorpc::Error> for ErrorReport {
    fn from(error: utxorpc::Error) -> Self {
        match error {
            utxorpc::Error::TransportError(err) => {
                ErrorReport::new(err.to_string(), "transport".to_string())
            }
            utxorpc::Error::GrpcError(status) => {
                ErrorReport::new(status.message().to_string(), "grpc".to_string())
            }
            utxorpc::Error::ParseError(message) => ErrorReport::new(message, "parse".to_string()),
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
