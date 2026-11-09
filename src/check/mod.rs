//! 依赖/引用 检查的公共实现.
//!
//! `conf` 和 `task` 都需要检查 task/f 的 引用/依赖:
//! (1) 被引用/依赖 的 task/f 必须存在.
//! (2) 不允许 循环依赖.
//!
//! 这里把与 具体错误类型无关的部分 (收集依赖边, 存在性检查, 环检测)
//! 抽到一处, 由调用方负责映射到自己的错误类型.
use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
};

use crate::{
    conf::{HasP, HasPD, PmtcF, PmtcTask, SrcP},
    te::TeError,
};

/// 依赖/引用 错误的 key-value 参数 (用于 友好错误显示, 尽力而为).
pub fn dep_err_params(t: &str, name: &str, target: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("t".to_string(), t.to_string());
    m.insert("name".to_string(), name.to_string());
    m.insert("dep".to_string(), target.to_string());
    m
}

/// 检测到的循环依赖.
#[derive(Debug, Clone)]
pub struct Cycle {
    pub node: String,
    pub target: String,
    pub src: SrcP,
}

impl Cycle {
    /// 生成错误说明.
    pub fn why(&self) -> String {
        format!("cycle: `{}` -> `{}`", self.node, self.target)
    }
}

/// 检查依赖图是否 无环 (有向无环图).
///
/// `edges`: `node -> (target, src)` 列表.
/// 检测到环返回 [`Cycle`].
pub fn check_dep_cycle(edges: &HashMap<String, Vec<(String, SrcP)>>) -> Result<(), Cycle> {
    fn dfs(
        node: &str,
        edges: &HashMap<String, Vec<(String, SrcP)>>,
        color: &mut HashMap<String, u8>,
    ) -> Result<(), Cycle> {
        // 1: 正在访问 (栈中), 2: 已完成
        color.insert(node.to_string(), 1);
        if let Some(list) = edges.get(node) {
            for (target, src) in list {
                match color.get(target).copied().unwrap_or(0) {
                    1 => {
                        return Err(Cycle {
                            node: node.to_string(),
                            target: target.clone(),
                            src: *src,
                        });
                    }
                    2 => {}
                    _ => dfs(target, edges, color)?,
                }
            }
        }
        color.insert(node.to_string(), 2);
        Ok(())
    }

    let mut color: HashMap<String, u8> = HashMap::new();
    let nodes: Vec<String> = edges.keys().cloned().collect();
    for node in nodes {
        if color.get(&node).copied().unwrap_or(0) == 0 {
            dfs(&node, edges, &mut color)?;
        }
    }
    Ok(())
}

/// 依赖/引用 缺失 (结构化, 供调用方映射到自己的错误类型).
#[derive(Debug, Clone)]
pub enum MissingRef {
    /// task `env_for` 引用的 task 不存在.
    TaskEnvFor {
        src: SrcP,
        name: String,
        target: String,
    },
    /// task `dep` 引用的 task 不存在.
    TaskDep {
        src: SrcP,
        name: String,
        target: String,
    },
    /// task `f` 引用的 f 不存在.
    TaskF {
        src: SrcP,
        name: String,
        target: String,
    },
    /// f `dep` 引用的 f 不存在.
    FDep {
        src: SrcP,
        name: String,
        target: String,
    },
}

impl MissingRef {
    /// 错误位置.
    pub fn src(&self) -> SrcP {
        match self {
            Self::TaskEnvFor { src, .. }
            | Self::TaskDep { src, .. }
            | Self::TaskF { src, .. }
            | Self::FDep { src, .. } => *src,
        }
    }

    /// `dep_err_params` 的 `t` 参数.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::TaskEnvFor { .. } => "env_for",
            Self::TaskDep { .. } => "dep",
            Self::TaskF { .. } => "f",
            Self::FDep { .. } => "dep",
        }
    }

    /// `name` / `target` (供 `dep_err_params` 使用).
    pub fn name_target(&self) -> (&str, &str) {
        match self {
            Self::TaskEnvFor { name, target, .. }
            | Self::TaskDep { name, target, .. }
            | Self::TaskF { name, target, .. }
            | Self::FDep { name, target, .. } => (name.as_str(), target.as_str()),
        }
    }

    /// 生成错误说明.
    pub fn why(&self) -> String {
        let (name, target) = self.name_target();
        match self {
            Self::TaskEnvFor { .. } => {
                format!("task `{}` env_for missing task `{}`", name, target)
            }
            Self::TaskDep { .. } => {
                format!("task `{}` depends on missing task `{}`", name, target)
            }
            Self::TaskF { .. } => {
                format!("task `{}` references missing f `{}`", name, target)
            }
            Self::FDep { .. } => {
                format!("f `{}` depends on missing f `{}`", name, target)
            }
        }
    }

    /// 生成错误参数.
    pub fn params(&self) -> HashMap<String, String> {
        let (name, target) = self.name_target();
        dep_err_params(self.kind(), name, target)
    }
}

/// 收集 task/f 的依赖边, 并检查被引用/依赖的 task/f 必须存在.
///
/// 返回 `(task_edges, f_edges)`; 不检查环 (环由 [`check_dep_cycle`] 处理).
///
/// `env_for` 与 `dep` 一并进入 `task_edges`, 因此两者的环都会被检测.
pub fn dep_edges(
    task: &HashMap<String, Vec<HasP<PmtcTask>>>,
    f: &HashMap<String, HasP<PmtcF>>,
) -> Result<
    (
        HashMap<String, Vec<(String, SrcP)>>,
        HashMap<String, Vec<(String, SrcP)>>,
    ),
    MissingRef,
> {
    let mut task_edges: HashMap<String, Vec<(String, SrcP)>> = HashMap::new();
    let mut f_edges: HashMap<String, Vec<(String, SrcP)>> = HashMap::new();

    for (name, tasks) in task {
        for ht in tasks {
            let t = ht.as_ref();

            if let Some(h) = &t.env_for {
                let target = h.as_ref();
                if !task.contains_key(target) {
                    return Err(MissingRef::TaskEnvFor {
                        src: h.src(),
                        name: name.clone(),
                        target: target.clone(),
                    });
                }
                task_edges
                    .entry(name.clone())
                    .or_default()
                    .push((target.clone(), h.src()));
            }

            for h in &t.dep {
                let target = h.as_ref();
                if !task.contains_key(target) {
                    return Err(MissingRef::TaskDep {
                        src: h.src(),
                        name: name.clone(),
                        target: target.clone(),
                    });
                }
                task_edges
                    .entry(name.clone())
                    .or_default()
                    .push((target.clone(), h.src()));
            }

            if let Some(h) = &t.f {
                let target = h.as_ref();
                if !f.contains_key(target) {
                    return Err(MissingRef::TaskF {
                        src: h.src(),
                        name: name.clone(),
                        target: target.clone(),
                    });
                }
            }
        }
    }

    for (name, hf) in f {
        let pf = hf.as_ref();
        for h in &pf.dep {
            let target = h.as_ref();
            if !f.contains_key(target) {
                return Err(MissingRef::FDep {
                    src: h.src(),
                    name: name.clone(),
                    target: target.clone(),
                });
            }
            f_edges
                .entry(name.clone())
                .or_default()
                .push((target.clone(), h.src()));
        }
    }

    Ok((task_edges, f_edges))
}

/// 把路径 字符串 转换为 UNIX 形式 (`/` 分隔).
///
/// - UNIX 平台: 空操作.
/// - Windows: 把所有 `\` 替换为 `/`.
///
/// 供路径处理代码在 输出 之前调用, 保证 pm-tc 内部路径 一律为 UNIX 形式.
pub fn to_unix_path(p: String) -> String {
    #[cfg(windows)]
    {
        p.replace('\\', "/")
    }
    #[cfg(not(windows))]
    {
        p
    }
}

/// 对路径做词法归一化 (不访问文件系统).
///
/// 折叠 `./`, 尽量消除 `../`, 使相对路径的等价写法得到相同的结果,
/// 用于判断 "同一文件".
///
/// 输入/输出 均为 UNIX 形式路径 (`/` 分隔); 内部使用 `std::path` 处理,
/// 输出前调用 [`to_unix_path`] 转换.
pub fn norm_path(p: &str) -> String {
    let mut out = PathBuf::new();
    for c in Path::new(p).components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => match out.components().next_back() {
                Some(Component::Normal(_)) => {
                    out.pop();
                }
                Some(Component::ParentDir) | None => {
                    out.push("..");
                }
                Some(Component::RootDir) => {}
                Some(_) => {
                    out.push("..");
                }
            },
            other => out.push(other.as_os_str()),
        }
    }
    to_unix_path(out.to_string_lossy().into_owned())
}

/// 把 `TeError` 包装为 带源位置 的错误 (供 conf/task 各自的错误类型复用).
pub fn te_err(src: SrcP, e: TeError) -> HasPD<TeError> {
    HasPD::new(src, e)
}

#[cfg(test)]
mod test;
