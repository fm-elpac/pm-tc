//! 友好错误显示 (国际化).
//!
//! 读取 [`Lang`] 并分发到 中文/英文 实现.
use std::{env, error::Error};

use crate::{
    ConfError, PmtcError, TaskError, TeError,
    conf::{SrcInfo, SrcP},
};

pub mod en;
pub mod zh;

/// 显示语言.
///
/// 目前仅支持 简体中文/英文.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// 英文
    En,
    /// 中文
    Zh,
}

/// 读取 `LANG` 环境变量, 判断 显示语言.
///
/// 如果 `LANG` 以 `zh` 开头, 返回 [`Lang::Zh`],
/// 其余情况 返回 [`Lang::En`].
pub fn get_lang() -> Lang {
    match env::var("LANG") {
        Ok(v) if v.starts_with("zh") => Lang::Zh,
        _ => Lang::En,
    }
}

/// 友好错误显示.
///
/// 检查 错误数据 是否为 pm-tc 库 自己产生的错误 (非 透传 错误).
/// 如果 可以处理, 返回 [`Some`] 字符串 (1 行 或 2 行), 否则返回 [`None`].
pub fn t_err(err: &Box<dyn Error>, src: &SrcInfo) -> Option<String> {
    match get_lang() {
        Lang::Zh => zh::t_err(err, src),
        Lang::En => en::t_err(err, src),
    }
}

/// pm-tc 库 自己产生的错误.
#[derive(Debug, Clone, Copy)]
pub(crate) enum OwnErr<'a> {
    Conf(&'a ConfError),
    Task(&'a TaskError),
    Te(&'a TeError),
}

/// 检查 错误数据 是否为 pm-tc 库 自己产生的错误.
pub(crate) fn classify<'a>(err: &'a (dyn Error + 'static)) -> Option<OwnErr<'a>> {
    if let Some(e) = err.downcast_ref::<ConfError>() {
        Some(OwnErr::Conf(e))
    } else if let Some(e) = err.downcast_ref::<TaskError>() {
        Some(OwnErr::Task(e))
    } else if let Some(e) = err.downcast_ref::<TeError>() {
        Some(OwnErr::Te(e))
    } else if let Some(e) = err.downcast_ref::<PmtcError>() {
        match e {
            PmtcError::Conf(e) => Some(OwnErr::Conf(e)),
            PmtcError::Te(e) => Some(OwnErr::Te(e)),
            PmtcError::Task(e) => Some(OwnErr::Task(e)),
        }
    } else {
        None
    }
}

/// 解析 源位置, 返回 `(文件路径, 行号, 列号)`.
///
/// 只要 错误数据 带有 `SrcP`, 就尽力给出位置.
/// 文件路径 获取失败 时使用 空字符串. 行号/列号 获取失败 显示 0.
pub(crate) fn pos(src: &SrcInfo, p: &SrcP) -> Option<(String, usize, usize)> {
    match src.info_col(p) {
        Ok(v) => Some(v),
        Err(_) => Some((String::new(), 0, 0)),
    }
}

/// 组合 显示结果: 有 位置 时 2 行, 否则 1 行.
pub(crate) fn render(pos: Option<(String, usize, usize)>, msg: impl Into<String>) -> String {
    let msg = msg.into();
    match pos {
        Some((path, line, col)) => format!("({}:{})  {}\n    {}", line, col, path, msg),
        None => msg,
    }
}
