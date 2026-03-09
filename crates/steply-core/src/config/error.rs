use std::error::Error;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum ConfigLoadError {
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },
    ParseYaml(serde_yaml::Error),
    Normalize(String),
    Validate(String),
    Assemble(String),
}

impl fmt::Display for ConfigLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadFile { path, source } => {
                write!(f, "failed to read yaml config {}: {source}", path.display())
            }
            Self::ParseYaml(source) => write!(f, "failed to parse yaml config: {source}"),
            Self::Normalize(message) => write!(f, "failed to normalize yaml config: {message}"),
            Self::Validate(message) => write!(f, "invalid yaml config: {message}"),
            Self::Assemble(message) => write!(f, "failed to assemble yaml config: {message}"),
        }
    }
}

impl Error for ConfigLoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ReadFile { source, .. } => Some(source),
            Self::ParseYaml(source) => Some(source),
            Self::Normalize(_) | Self::Validate(_) | Self::Assemble(_) => None,
        }
    }
}
