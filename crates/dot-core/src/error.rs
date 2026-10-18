//! dot error type.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, DotError>;

#[derive(Debug, Error)]
pub enum DotError {
    #[error("config file not found at {path}: {hint}")]
    ConfigMissing {
        path: std::path::PathBuf,
        hint: &'static str,
    },

    #[error("invalid config at {path}: {source}")]
    ConfigInvalid {
        path: std::path::PathBuf,
        source: toml::de::Error,
    },

    #[error("module not found: {0}")]
    ModuleNotFound(String),

    #[error("module {module} is not supported on {os}")]
    UnsupportedOs { module: String, os: String },

    #[error("target {0} is outside of $HOME")]
    TargetOutsideHome(std::path::PathBuf),

    #[error("module {module} does not define template {template}")]
    TemplateNotFound { module: String, template: String },

    #[error("failed to render template {template} in module {module}: {source}")]
    RenderError {
        module: String,
        template: String,
        source: minijinja::Error,
    },

    #[error("io error on {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },

    #[error("manifest at {path} is corrupt: {source}")]
    ManifestCorrupt {
        path: std::path::PathBuf,
        source: toml::de::Error,
    },

    #[error("backup {0} not found")]
    BackupNotFound(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl DotError {
    pub fn io(path: impl Into<std::path::PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
