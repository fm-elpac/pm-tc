//! 错误处理
use std::{collections::HashMap, error::Error, fmt};

use crate::{conf::HasPD, te::TeError};

/// task 错误.
///
/// 仿照 [`crate::conf::ConfError`] 的风格:
/// - `HasPD<why>` 是 错误位置 (配合 `SrcInfo` 显示 文件名/行号, 尽力而为).
/// - `why: String` 是 库内置的 简洁 (英文) 错误说明.
/// - `HashMap<String, String>` 保存 关键错误信息 key-value, 用于 友好错误显示.
///
/// 注意: `HashMap<String, String>` 中的 key 不计入 库 的稳定 API.
#[derive(Debug, Clone)]
pub enum TaskError {
    /// 引用的 task 不存在 (`dep` / `env_for`)
    DepT((HasPD<String>, HashMap<String, String>)),
    /// 引用的 f 不存在
    F((HasPD<String>, HashMap<String, String>)),
    /// f 定义 错误 (比如 `skip_include_eval` 缺少 `file_eval`)
    FDef((HasPD<String>, HashMap<String, String>)),
    /// 命令行参数 (arg) 数量 不对
    Arg((HasPD<String>, HashMap<String, String>)),
    /// 循环依赖 (`dep` / `env_for`)
    DepCycle((HasPD<String>, HashMap<String, String>)),
    /// 文件写入 冲突 (同一文件 多种/多次 写入)
    FileWrite((HasPD<String>, HashMap<String, String>)),
    /// te 模板 错误
    Te(HasPD<TeError>),
}

impl Error for TaskError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Te(p) => Some(p.as_ref()),
            _ => None,
        }
    }
}

/// 此处的显示方式只是 缺省/默认 显示, 适用于 开发/调试.
/// 调用者 可以使用对用户更友好的显示方式.
impl fmt::Display for TaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DepT(p) => write!(f, "TaskError::DepT {}", p.0),
            Self::F(p) => write!(f, "TaskError::F {}", p.0),
            Self::FDef(p) => write!(f, "TaskError::FDef {}", p.0),
            Self::Arg(p) => write!(f, "TaskError::Arg {}", p.0),
            Self::DepCycle(p) => write!(f, "TaskError::DepCycle {}", p.0),
            Self::FileWrite(p) => write!(f, "TaskError::FileWrite {}", p.0),
            Self::Te(p) => write!(f, "TaskError::Te {}", p),
        }
    }
}
