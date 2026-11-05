//! pm-tc: 胖喵模板配置工具.
//!
//! 解析/检查 `pm_tc.conf.toml`, 并按 依赖关系 生成/执行 任务计划.
#![deny(unsafe_code)]

#[cfg(feature = "cli")]
pub mod cli;

pub mod conf;
pub mod te;

#[cfg(feature = "f")]
pub mod f;
#[cfg(feature = "task")]
pub mod task;

mod check;
mod error;

pub use conf::{ConfError, PmtcConf};
pub use error::PmtcError;
pub use te::{PmtcEnv, TeError};

#[cfg(feature = "task")]
pub use task::{CheckEnv, PlanDebug, PmtcRunList, PmtcRunOp, TaskDebug, TaskError};
