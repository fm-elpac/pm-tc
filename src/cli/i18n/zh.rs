//! 中文 友好错误信息.
use std::{collections::HashMap, error::Error};

use crate::{ConfError, TaskError, TeError, conf::SrcInfo};

use super::{OwnErr, classify, pos, render};

/// 中文 友好错误显示.
///
/// 见 [`super::t_err`].
pub fn t_err(err: &Box<dyn Error>, src: &SrcInfo) -> Option<String> {
    match classify(err.as_ref())? {
        OwnErr::Conf(c) => conf_msg(c, src),
        OwnErr::Task(t) => task_msg(t, src),
        OwnErr::Te(e) => Some(render(None, te_msg(e))),
    }
}

/// 读取错误参数 (尽力而为).
fn g<'a>(m: &'a HashMap<String, String>, k: &str) -> Option<&'a str> {
    m.get(k).map(|s| s.as_str())
}

fn conf_msg(e: &ConfError, src: &SrcInfo) -> Option<String> {
    match e {
        ConfError::FileId(id) => Some(render(None, format!("内部错误: 无效的配置文件编号 {}", id))),
        ConfError::Toml(p) => Some(render(
            pos(src, &p.src()),
            format!("toml 解析错误: {}", p.as_ref().message()),
        )),
        ConfError::Te(p) => Some(render(pos(src, &p.src()), te_msg(p.as_ref()))),
        ConfError::ConfVersion((p, m)) => {
            let msg = match g(m, "t") {
                Some("missing") => "缺少配置文件格式版本 `pm_tc`".to_string(),
                Some("unsupported") => format!(
                    "不支持的配置文件格式版本 {} (支持 {})",
                    g(m, "version").unwrap_or("?"),
                    g(m, "supported").unwrap_or("?")
                ),
                _ => p.as_ref().clone(),
            };
            Some(render(pos(src, &p.src()), msg))
        }
        ConfError::Key((p, m)) => {
            let msg = match g(m, "t") {
                Some("unknown") => format!("未知的 toml 键 `{}`", g(m, "key").unwrap_or("?")),
                Some("duplicate") => format!("重复的 toml 键 `{}`", g(m, "key").unwrap_or("?")),
                _ => p.as_ref().clone(),
            };
            Some(render(pos(src, &p.src()), msg))
        }
        ConfError::Value((p, m)) => {
            let msg = value_msg(m, p.as_ref());
            Some(render(pos(src, &p.src()), msg))
        }
        ConfError::IncludeDup((p, m)) => Some(render(
            pos(src, &p.src()),
            format!("重复的 `[[include]]`: `{}`", g(m, "file").unwrap_or("?")),
        )),
        ConfError::IncludeCycle((p, m)) => Some(render(
            pos(src, &p.src()),
            format!("循环 `[[include]]`: `{}`", g(m, "file").unwrap_or("?")),
        )),
        ConfError::DepCycle((p, _)) => Some(render(pos(src, &p.src()), "循环依赖".to_string())),
        ConfError::FA((p, m)) => {
            let msg = match g(m, "t") {
                Some("missing") => format!("缺少 `{}`", g(m, "key").unwrap_or("?")),
                Some("unknown") => {
                    format!("未知的动作类型 `a`: `{}`", g(m, "action").unwrap_or("?"))
                }
                _ => p.as_ref().clone(),
            };
            Some(render(pos(src, &p.src()), msg))
        }
        ConfError::FAEmpty((p, m)) => {
            let msg = match g(m, "t") {
                Some("empty") => format!("`{}` 不能为空", g(m, "key").unwrap_or("?")),
                Some("element") => format!("`{}` 的元素不能为空", g(m, "key").unwrap_or("?")),
                _ => p.as_ref().clone(),
            };
            Some(render(pos(src, &p.src()), msg))
        }
        ConfError::DepT((p, m)) => {
            let msg = dep_msg(m, p.as_ref(), "任务");
            Some(render(pos(src, &p.src()), msg))
        }
        ConfError::DepF((p, m)) => {
            let msg = dep_msg(m, p.as_ref(), "功能");
            Some(render(pos(src, &p.src()), msg))
        }
        ConfError::F((p, m)) => {
            let msg = match g(m, "t") {
                Some("f") => format!(
                    "任务 `{}` 引用的功能 `{}` 不存在",
                    g(m, "name").unwrap_or("?"),
                    g(m, "dep").unwrap_or("?")
                ),
                _ => p.as_ref().clone(),
            };
            Some(render(pos(src, &p.src()), msg))
        }
        ConfError::FDup((p, m)) => Some(render(
            pos(src, &p.src()),
            format!("功能 `{}` 重复", g(m, "name").unwrap_or("?")),
        )),
    }
}

fn task_msg(e: &TaskError, src: &SrcInfo) -> Option<String> {
    match e {
        TaskError::DepT((p, m)) => {
            let msg = if m.is_empty() {
                p.as_ref().clone()
            } else {
                dep_msg(m, p.as_ref(), "任务")
            };
            Some(render(pos(src, &p.src()), msg))
        }
        TaskError::F((p, m)) => {
            let msg = match g(m, "t") {
                Some("f") => format!(
                    "任务 `{}` 引用的功能 `{}` 不存在",
                    g(m, "name").unwrap_or("?"),
                    g(m, "dep").unwrap_or("?")
                ),
                Some("dep") => format!(
                    "功能 `{}` 依赖的功能 `{}` 不存在",
                    g(m, "name").unwrap_or("?"),
                    g(m, "dep").unwrap_or("?")
                ),
                _ => p.as_ref().clone(),
            };
            Some(render(pos(src, &p.src()), msg))
        }
        TaskError::FDef((p, _)) => Some(render(pos(src, &p.src()), p.as_ref().clone())),
        TaskError::Arg((p, _)) => Some(render(pos(src, &p.src()), p.as_ref().clone())),
        TaskError::DepCycle((p, _)) => Some(render(pos(src, &p.src()), "循环依赖".to_string())),
        TaskError::FileWrite((p, _)) => Some(render(pos(src, &p.src()), p.as_ref().clone())),
        TaskError::Te(p) => Some(render(pos(src, &p.src()), te_msg(p.as_ref()))),
    }
}

fn value_msg(m: &HashMap<String, String>, why: &str) -> String {
    match g(m, "t") {
        Some("prefix") => format!("引用缺少前缀: `{}`", g(m, "value").unwrap_or("?")),
        Some("extra") => format!(
            "动作 `{}` 不允许参数 `{}`",
            g(m, "action").unwrap_or("?"),
            g(m, "param").unwrap_or("?")
        ),
        Some("int") => match g(m, "key") {
            Some(k) => format!("`{}` 必须是整数", k),
            None => "必须是整数".to_string(),
        },
        Some("range") => format!(
            "`{}` 超出范围: {}",
            g(m, "key").unwrap_or("?"),
            g(m, "value").unwrap_or("?")
        ),
        Some("missing") => match g(m, "key") {
            Some(k) => format!("缺少 `{}`", k),
            None => "缺少字段".to_string(),
        },
        Some("dup") => format!("重复的 `{}`", g(m, "name").unwrap_or("?")),
        Some("path") => "`path_eval` 与 `path_eval_a` 不能同时使用".to_string(),
        Some("mkdir") => "`mkdir` 仅支持单个 `path_eval`".to_string(),
        Some("octal") => format!(
            "`{}` 必须是八进制数 (如 `700`): {}",
            g(m, "key").unwrap_or("?"),
            g(m, "value").unwrap_or("?")
        ),
        Some("empty") => format!("`{}` 不能为空", g(m, "key").unwrap_or("?")),
        // serde 生成的错误 没有 key-value 参数, 中文 尽力而为 使用英文 why.
        Some("custom") | Some("type") | Some("invalid") | Some("length") | Some("unsupported")
        | Some("value") | Some("str") | Some("array") | Some("table") => why.to_string(),
        _ => why.to_string(),
    }
}

fn dep_msg(m: &HashMap<String, String>, why: &str, kind: &str) -> String {
    match g(m, "t") {
        Some("env_for") => format!(
            "{} `{}` 的 `env_for` 引用的 {} `{}` 不存在",
            kind,
            g(m, "name").unwrap_or("?"),
            kind,
            g(m, "dep").unwrap_or("?")
        ),
        Some("dep") => format!(
            "{} `{}` 依赖的 {} `{}` 不存在",
            kind,
            g(m, "name").unwrap_or("?"),
            kind,
            g(m, "dep").unwrap_or("?")
        ),
        _ => why.to_string(),
    }
}

fn te_msg(e: &TeError) -> String {
    match e {
        TeError::Escape((pos, c)) => match c {
            Some(c) => format!("无效的转义序列 (位置 {}): `{}`", pos, c),
            None => format!("无效的转义序列 (位置 {})", pos),
        },
        TeError::NameChar((pos, c)) => {
            format!("变量名包含不允许的字符 (位置 {}): `{}`", pos, c)
        }
        TeError::NameNFKC((pos, before, after)) => {
            format!(
                "变量名 NFKC 后发生变化 (位置 {}): `{}` -> `{}`",
                pos, before, after
            )
        }
        TeError::NameEmpty(pos) => format!("变量名为空 (位置 {})", pos),
        TeError::VE((pos, t)) => format!("缺少匹配的 `]` (位置 {}): `{}`", pos, t),
        TeError::VF((pos, t)) => format!("模板语法错误 (位置 {}): `{}`", pos, t),
        TeError::EvalRec((r, _)) => format!("模板递归次数超过限制 {}", r),
    }
}
