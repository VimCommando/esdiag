use super::{DiagnosticManifest, Manifest};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataSet {
    DiagnosticManifest,
    Manifest,
}

impl std::fmt::Display for DataSet {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DiagnosticManifest => write!(fmt, "diagnostic_manifest"),
            Self::Manifest => write!(fmt, "manifest"),
        }
    }
}

impl From<DiagnosticManifest> for DataSet {
    fn from(_: DiagnosticManifest) -> Self {
        Self::DiagnosticManifest
    }
}

impl From<Manifest> for DataSet {
    fn from(_: Manifest) -> Self {
        Self::Manifest
    }
}
