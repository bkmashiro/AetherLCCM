use std::fmt::{Display, Formatter};

#[derive(Debug, Clone)]
pub enum ProtocolError {
    NotFound {
        what: String,
    },
    Validation {
        reason: String,
    },
    Conflict {
        reason: String,
    },
    Crypto {
        reason: String,
    },
    NotImplemented {
        feature: String,
    },
}

pub type ProtocolResult<T> = Result<T, ProtocolError>;

impl Display for ProtocolError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { what } => write!(f, "not found: {what}"),
            Self::Validation { reason } => write!(f, "validation failed: {reason}"),
            Self::Conflict { reason } => write!(f, "conflict: {reason}"),
            Self::Crypto { reason } => write!(f, "crypto failed: {reason}"),
            Self::NotImplemented { feature } => write!(f, "not implemented: {feature}"),
        }
    }
}

impl std::error::Error for ProtocolError {}

