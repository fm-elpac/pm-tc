//! 任务/依赖
//!
//! 在 `task::plan()` 与 `task::run()` 执行期间, pm-tc 假定外部环境保持不变:
//! 没有其它程序或用户操作与 pm-tc 冲突或并发执行. 调用者应保证这一点;
//! pm-tc 不处理由此导致的故障或 BUG.
use std::{
    collections::{BTreeMap, HashSet},
    env,
    error::Error,
};

use chrono::Local;
use serde::Serialize;

#[cfg(feature = "f")]
use crate::f;
use crate::te::{PMTC_ENV_LABEL_BUILTIN, PMTC_ENV_LABEL_SYS, PmtcEnv};

mod error;
mod p;

pub use error::TaskError;
pub use p::plan;

/// 特殊变量 `@n` (用于 `[[t.*]] arg = n`)
pub const PMTC_ENV_AT: &'static str = "@";

/// 内置变量 `date`: 当前日期 `yyyyMMdd` (比如 `20261029`)
pub const PMTC_ENV_DATE: &'static str = "date";

/// 单个执行动作
#[derive(Debug, Clone, Serialize)]
pub enum PmtcRunOp {
    /// 输出日志
    Log(Vec<String>),
    /// 执行外部命令
    Exec(Vec<String>),
    /// 调用 shell 执行命令
    ExecSh(String),
    /// mkdir -p (path)
    Mkdir(String),
    /// chmod (path, mode)
    Chmod((String, u32)),
    /// cat (path)
    Cat(String),
    /// echo (text)
    Echo(String),
    /// 替换文件内容 (path, text)
    FileReplace((String, String)),
    /// 追加写入文件 (path, text)
    FileAppend((String, String)),
}

/// 计划执行的 命令 (动作) 列表
#[derive(Debug, Clone)]
pub struct PmtcRunList {
    list: Vec<PmtcRunOp>,
}

impl PmtcRunList {
    /// 空动作列表
    pub fn new() -> Self {
        Self { list: Vec::new() }
    }

    /// 添加动作
    pub fn add(&mut self, op: PmtcRunOp) -> &mut Self {
        self.list.push(op);
        self
    }

    /// 转换类型
    pub fn to_vec(&self) -> Vec<PmtcRunOp> {
        self.list.clone()
    }
}

impl IntoIterator for PmtcRunList {
    type Item = PmtcRunOp;
    type IntoIter = std::vec::IntoIter<PmtcRunOp>;

    fn into_iter(self) -> Self::IntoIter {
        self.list.into_iter()
    }
}

/// 回调接口: 检查环境现状 (读取文件 等)
pub trait CheckEnv {
    /// 指定文件 (目录) 是否存在
    fn file_exist(&mut self, path: &str) -> Result<bool, Box<dyn Error>>;
    /// 读取文件内容 (utf-8 编码)
    fn file_read(&mut self, path: &str) -> Result<String, Box<dyn Error>>;
}

/// 用于 `task::plan` 调试
#[derive(Debug, Clone, Serialize)]
pub struct PlanDebug {
    /// 执行 每个任务 之前的 基础/只读 变量
    pub env0: PmtcEnv,
    /// 初始化阶段 未知变量
    pub u: HashSet<String>,
    /// 执行的 每个任务.
    ///
    /// 顺序与 `PmtcRunList` 中的 执行顺序 一致.
    pub task: Vec<TaskDebug>,
}

/// 用于 `task::plan` 调试 (单个任务)
#[derive(Debug, Clone, Serialize)]
pub struct TaskDebug {
    /// 任务名称
    pub name: String,
    /// 最终 变量 (集) (有序)
    pub env: BTreeMap<String, String>,
    /// 使用的变量 `use`
    pub env_use: HashSet<(String, u8)>,
    /// 未知变量
    pub u: HashSet<String>,
}

/// 生成 内置变量 (集)
pub fn env_builtin() -> PmtcEnv {
    let mut o = PmtcEnv::new();

    // `date`: 当前日期
    let date = Local::now().format("%Y%m%d").to_string();
    o.set_label(PMTC_ENV_DATE, &date, PMTC_ENV_LABEL_BUILTIN);

    o
}

/// 生成 系统 (进程) 环境变量 (集)
/// 只收集 能够被 utf-8 解码 的 环境变量 名称/值, 忽略错误.
pub fn env_sys() -> PmtcEnv {
    let mut o = PmtcEnv::new();

    for (name, value) in env::vars_os() {
        if let (Ok(name), Ok(value)) = (name.into_string(), value.into_string()) {
            o.set_label(&name, &value, PMTC_ENV_LABEL_SYS);
        }
    }
    o
}

/// 实际执行 (默认执行器)
#[cfg(feature = "f")]
pub fn run(list: PmtcRunList) -> Result<(), Box<dyn Error>> {
    for i in list {
        match i {
            PmtcRunOp::Log(a) => f::log(&a)?,
            PmtcRunOp::Exec(a) => f::exec(&a)?,
            PmtcRunOp::ExecSh(c) => f::exec_sh(&c)?,
            PmtcRunOp::Mkdir(path) => f::mkdir(&path)?,
            PmtcRunOp::Chmod((path, mode)) => f::chmod(&path, mode)?,
            PmtcRunOp::Cat(path) => f::cat(&path)?,
            PmtcRunOp::Echo(text) => f::echo(&text)?,
            PmtcRunOp::FileReplace((path, text)) => f::file_replace(&path, &text)?,
            PmtcRunOp::FileAppend((path, text)) => f::file_append(&path, &text)?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod test;
