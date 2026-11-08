//! 错误处理.
//!
//! ## 维护约定
//!
//! 本模块顶部文档必须完整说明 库中 每一个错误 携带的 `HashMap<String, String>`
//! 的 key 情况, 使用者 只需阅读本模块文档即可, 无需再去阅读 源代码的每一处.
//! 新增错误 或 修改现有错误的 key 时, 必须同步更新这里的说明.
//!
//! ## 概述
//!
//! 库中的错误 由 [`PmtcError`] 汇总, 分别包装:
//!
//! - [`crate::conf::ConfError`]
//! - [`crate::te::TeError`]
//! - `crate::task::TaskError` (仅在 `task` 特性启用时)
//!
//! 只有 `ConfError` 与 `TaskError` 的部分变体 携带 `HashMap<String, String>`
//! (用于 友好错误显示). `TeError` 与 [`PmtcError`] 不携带.
//! 这些 key 不计入 库的稳定 API, 且 友好错误信息 只是 尽力而为.
//!
//! 通用 key 约定:
//!
//! - `t`: 区分 同一变体 复用的 不同情形 (字符串).
//! - `key`: 出错的 toml key (字段名).
//! - `value`: 出错的值.
//! - `name`: 出错的名字 (如 `[[f]]` 名称).
//! - `action`: 动作类型 `a`.
//! - `param`: 动作 `a` 不允许使用的 参数名.
//! - `file`: 相关文件路径 (`[[include]]`).
//! - `dep`: 依赖/引用 指向的 目标名.
//! - `version`: 当前 配置文件 格式 版本号.
//! - `supported`: 支持的 配置文件 格式 版本号.
//!
//! ## ConfError
//!
//! 无 HashMap 的变体: `FileId`, `Toml`, `Te`.
//!
//! 有 HashMap 的变体:
//!
//! - `ConfVersion`: `t` = `"missing"` (无其他 key) 或 `"unsupported"`
//!   (另有 `version`, `supported`).
//! - `Key`: `t` = `"unknown"` 或 `"duplicate"`, 另有 `key`.
//! - `Value`: `t` 与 额外 key 的对应 见下表.
//!
//!   | `t` | 额外 key |
//!   | --- | --- |
//!   | `prefix` | `value` |
//!   | `extra` | `action`, `param` |
//!   | `int` | 无 或 `key` |
//!   | `range` | `key`, `value` |
//!   | `missing` | 无 或 `key` |
//!   | `dup` | `name` |
//!   | `path` | 无 |
//!   | `mkdir` | 无 |
//!   | `octal` | `key`, `value` |
//!   | `custom` | 无 |
//!   | `type` | 无 |
//!   | `invalid` | 无 |
//!   | `length` | 无 |
//!   | `unsupported` | 无 |
//!   | `value` | 无 |
//!   | `str` | 无 |
//!   | `array` | 无 |
//!   | `table` | 无 |
//!
//! - `IncludeDup`: `t` = `"dup"`, 另有 `file`.
//! - `IncludeCycle`: 只有 `file` (无 `t`).
//! - `DepCycle`: 无 key (空 map).
//! - `FA`: `t` = `"missing"` (另有 `key`) 或 `"unknown"` (另有 `action`).
//! - `FAEmpty`: `t` = `"empty"` 或 `"element"`, 另有 `key`.
//! - `DepT`: `t` = `"env_for"` 或 `"dep"`, 另有 `name`, `dep`.
//! - `DepF`: `t` = `"dep"`, 另有 `name`, `dep`.
//! - `F`: `t` = `"f"`, 另有 `name`, `dep`.
//! - `FDup`: 当前未构造 (保留), 无 key.
//!
//! ## TaskError
//!
//! 无 HashMap 的变体: `Te`.
//!
//! 有 HashMap 的变体:
//!
//! - `DepT`: 空 map, 或 `t` = `"env_for"` / `"dep"` (另有 `name`, `dep`).
//! - `F`: 空 map, 或 `t` = `"f"` / `"dep"` (另有 `name`, `dep`).
//! - `FDef`: 空 map.
//! - `Arg`: 空 map.
//! - `DepCycle`: 空 map.
//! - `FileWrite`: 空 map.
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
