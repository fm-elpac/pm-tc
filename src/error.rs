//! 错误处理
use std::{error::Error, fmt};

#[cfg(feature = "task")]
use crate::TaskError;
use crate::{ConfError, TeError};

/// pm-tc 错误
#[derive(Debug, Clone)]
pub enum PmtcError {
    /// conf 错误
    Conf(ConfError),
    /// te 错误
    Te(TeError),
    /// task 错误
    #[cfg(feature = "task")]
    Task(TaskError),
}

impl From<ConfError> for PmtcError {
    fn from(value: ConfError) -> Self {
        Self::Conf(value)
    }
}

impl From<TeError> for PmtcError {
    fn from(value: TeError) -> Self {
        Self::Te(value)
    }
}

#[cfg(feature = "task")]
impl From<TaskError> for PmtcError {
    fn from(value: TaskError) -> Self {
        Self::Task(value)
    }
}

impl Error for PmtcError {}

impl fmt::Display for PmtcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conf(v) => write!(f, "{}", v),
            Self::Te(v) => write!(f, "{}", v),
            #[cfg(feature = "task")]
            Self::Task(v) => write!(f, "{}", v),
        }
    }
}
