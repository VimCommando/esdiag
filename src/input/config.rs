pub struct Config {
    pub namespace: String,
    pub debug: bool,
    pub log_level: LogLevel,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            namespace: "esdiag".to_string(),
            debug: false,
            log_level: LogLevel::Info,
        }
    }
}

pub enum LogLevel {
    Debug,
    Verbose,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "debug" => Self::Debug,
            "verbose" => Self::Verbose,
            "info" => Self::Info,
            "warn" => Self::Warn,
            "error" => Self::Error,
            _ => Self::Info,
        }
    }
}
