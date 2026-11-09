//! 配置文件 解析/加载
//!
//! 原则: 除非 实现特别复杂 (动态执行), 否则应该在 配置 阶段尽可能多的做错误检查,
//! 而不是把 错误 推迟到 执行 阶段.
//!
//! 路径约定: 配置文件 中 的 路径 与 本模块 内部 路径 均使用 UNIX 形式 (`/` 分隔),
//! 即使 运行在 Windows 上 (见 `check::norm_path`).
//!
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    path::Path,
};

use crate::{
    check::{MissingRef, check_dep_cycle, dep_edges, norm_path, to_unix_path},
    te::PmtcEnvList,
};

mod error;
mod p;
mod p1;

pub use error::{ConfError, HasP, HasPD, SrcInfo, SrcP};
pub use p::parse;

/// 配置文件 格式 版本号.
///
/// 当前固定为 (`pm_tc.conf.toml`):
///
/// ```toml
/// pm_tc = 1
/// ```
pub const PMTC_CONF_VERSION: u32 = 1;

/// 配置文件 默认文件名 (当前目录 加载)
pub const PMTC_DEFAULT_FILE_CONF: &'static str = "pm_tc.conf.toml";
/// 可使用 `PMTC_CONF` 环境变量 指定 配置文件
pub const ENV_PMTC_CONF: &'static str = "PMTC_CONF";
/// 保存的变量 默认文件名 (当前目录 加载)
pub const PMTC_DEFAULT_FILE_ENV: &'static str = "pm_tc.env";
/// 可使用 `PMTC_ENV` 环境变量 指定 保存变量的文件
pub const ENV_PMTC_ENV: &'static str = "PMTC_ENV";

/// 默认值: 执行模板时, 允许的最大递归深度
pub const PMTC_DEFAULT_EVAL_REC: u16 = 16;
/// 可使用 `PMTC_REC` 环境变量 指定 允许的最大递归深度
pub const ENV_PMTC_REC: &'static str = "PMTC_REC";
/// 可使用 `PMTC_JSON` 环境变量 指定 JSON 输出 (`1` 启用)
pub const ENV_PMTC_JSON: &'static str = "PMTC_JSON";

/// pm-tc 单个 配置文件 (*.toml).
///
/// `SrcP` (`HasP`) 用于标记 源文件 位置 (用于 报错).
#[derive(Debug, Clone)]
pub struct PmtcConfFile {
    /// 配置文件 格式 版本号
    pub pm_tc: u32,
    /// 执行模板时, 允许的最大递归深度.
    ///
    /// `None` 表示 当前配置文件 没有设置 `eval_rec`.
    /// 具体取哪个值, 由 [`load`] 处理 (仅 主配置文件 的 `eval_rec` 生效).
    pub eval_rec: Option<HasP<u16>>,
    /// `[[include]]` 文件列表
    pub include: Vec<HasP<String>>,
    /// `[env]` 变量定义.
    /// 按照 配置文件 中出现的 顺序.
    pub env: PmtcEnvList,
    /// `[[t.*]]` 任务 (task) 定义.
    /// 按照 配置文件 中出现的 顺序.
    ///
    /// 格式为 `[[t.NAME]]`, 同名 task 可以有多个.
    /// `HashMap<NAME, _>`
    pub task: HashMap<String, Vec<HasP<PmtcTask>>>,
    /// `[[f]]` 功能 (f) 定义.
    /// (配置文件中出现的 顺序无关)
    /// 不允许重名.
    ///
    /// `HashMap<name, _>`
    pub f: HashMap<String, HasP<PmtcF>>,
}

/// 配置文件 `[[t.*]]` (任务) 定义
#[derive(Debug, Clone)]
pub struct PmtcTask {
    /// 保留, 用于调试. `[[t.name]]` 形式.
    pub _name: String,
    /// 定义的 变量 (值): `env:NAME` 形式
    pub env: HashMap<String, HasP<String>>,
    /// `env_for` 引用 提供变量 的 task.
    /// (解析时 要求 `t.` 前缀, 此处 已去除前缀)
    pub env_for: Option<HasP<String>>,
    /// `f` 调用的 `[[f]]`.
    /// 值 直接 是 f 的 名称 (不带 `f.` 前缀), 由 [`load`] 阶段 统一检查.
    pub f: Option<HasP<String>>,
    /// `dep` 依赖的 task.
    /// (解析时 要求 `t.` 前缀, 此处 已去除前缀)
    pub dep: Vec<HasP<String>>,
    /// `arg` 使用 命令行参数 (的个数, 默认 0). `[@1]` 形式.
    pub arg: HasP<usize>,
}

/// 配置文件 `[[f]]` 定义 (解析后 形式)
#[derive(Debug, Clone)]
pub struct PmtcF {
    /// `name` 保留, 用于 调试
    pub _name: String,
    /// `once` 只执行一次, 默认 false
    pub once: bool,
    /// `skip_file_eval` 跳过: 如果文件存在.
    pub skip_file_eval: Option<HasP<String>>,
    /// `skip_include_eval` 跳过: 如果文件内容包含.
    pub skip_include_eval: Option<HasP<String>>,
    /// `dep` 依赖的 f.
    /// (解析时 要求 `f.` 前缀, 此处 已去除前缀)
    pub dep: Vec<HasP<String>>,
    /// 要执行的动作
    pub a: Option<HasP<PmtcFA>>,
}

/// 配置文件 `f.a` 动作类型
#[derive(Debug, Clone)]
pub enum PmtcFA {
    /// log 输出日志
    ///
    /// Log(log_eval)
    Log(Vec<HasP<String>>),
    /// `exec` 执行外部命令.
    ///
    /// Exec(exec_eval)
    Exec(HasP<String>),
    /// `exec_sh` 调用 shell 执行命令.
    ///
    /// ExecSh(exec_eval)
    ExecSh(HasP<String>),
    /// echo
    ///
    /// Echo(text_eval)
    Echo(HasP<String>),
    /// cat
    ///
    /// Cat(file_eval)
    Cat(HasP<String>),
    /// mkdir
    ///
    /// Mkdir(path_eval)
    Mkdir(HasP<String>),
    /// chmod
    Chmod(PmtcFChmod),
    /// file 替换文件
    File(PmtcFFileWrite),
    /// file_append 追加写入
    FileAppend(PmtcFFileWrite),
    /// file_merge 合并写入 (类似 `cat` 合并, 按顺序)
    FileMerge(PmtcFFileWrite),
}

/// 配置文件 `f.a` = `chmod`
#[derive(Debug, Clone)]
pub struct PmtcFChmod {
    /// `mod`
    pub mode: HasP<u32>,
    /// `path_eval`
    pub path_eval: Vec<HasP<String>>,
}

/// 配置文件 `f.a` = `file`, `file_append`, `file_merge`
#[derive(Debug, Clone)]
pub struct PmtcFFileWrite {
    /// `file_eval` 文件路径
    pub file_eval: HasP<String>,
    /// `text_eval` 文本 (文件内容)
    pub text_eval: HasP<String>,
}

/// pm-tc 完整配置 (处理 `[[include]]`)
#[derive(Debug, Clone)]
pub struct PmtcConf {
    /// 实际使用的 `eval_rec` 值
    pub eval_rec: u16,
    /// 所有 配置文件 的 `[env]` 变量定义 (按顺序)
    pub env: Vec<PmtcEnvList>,
    /// 所有 配置文件 的 `[[t.*]]` 定义 (按顺序).
    ///
    /// `HashMap<NAME, _>`
    pub task: HashMap<String, Vec<HasP<PmtcTask>>>,
    /// 所有 配置文件 的 `[[f]]` 定义 (顺序无关).
    ///
    /// `HashMap<NAME, _>`
    pub f: HashMap<String, HasP<PmtcF>>,
    /// 原始 配置文件 数据 (用于 报错)
    pub src: SrcInfo,
}

/// 回调接口: 读取 配置文件
pub trait ReadConfFile {
    /// 读取指定的配置文件
    fn read(&mut self, path: &str) -> Result<String, Box<dyn Error>>;
}

/// 检查 task/f 的 引用/依赖:
/// (1) 被引用/依赖 的 task/f 必须存在.
/// (2) 不允许 循环依赖.
fn check_task_f(
    task: &HashMap<String, Vec<HasP<PmtcTask>>>,
    f: &HashMap<String, HasP<PmtcF>>,
) -> Result<(), ConfError> {
    let (task_edges, f_edges) = dep_edges(task, f).map_err(|m| match &m {
        MissingRef::TaskEnvFor { .. } | MissingRef::TaskDep { .. } => {
            ConfError::DepT((HasPD::new(m.src(), m.why()), m.params()))
        }
        MissingRef::TaskF { .. } => ConfError::F((HasPD::new(m.src(), m.why()), m.params())),
        MissingRef::FDep { .. } => ConfError::DepF((HasPD::new(m.src(), m.why()), m.params())),
    })?;

    check_dep_cycle(&task_edges)
        .map_err(|c| ConfError::DepCycle((HasPD::new(c.src, c.why()), HashMap::new())))?;
    check_dep_cycle(&f_edges)
        .map_err(|c| ConfError::DepCycle((HasPD::new(c.src, c.why()), HashMap::new())))?;
    Ok(())
}

/// 加载过程中的 状态.
struct Loader<'a, 'b, R: ReadConfFile> {
    r: &'a mut R,
    src: &'b mut SrcInfo,
    /// 按 读取顺序 保存的 已解析 配置文件 (下标 == file_id).
    confs: Vec<PmtcConfFile>,
    /// 加载顺序 (file_id, 后序).
    order: Vec<usize>,
    /// 已完成 加载 的 配置文件 (归一化 路径, UNIX 形式).
    loaded: HashSet<String>,
    /// 当前 递归访问 栈 中的 配置文件 (用于 检测 循环, UNIX 形式).
    visiting: HashSet<String>,
}

impl<'a, 'b, R: ReadConfFile> Loader<'a, 'b, R> {
    fn new(r: &'a mut R, src: &'b mut SrcInfo) -> Self {
        Self {
            r,
            src,
            confs: Vec::new(),
            order: Vec::new(),
            loaded: HashSet::new(),
            visiting: HashSet::new(),
        }
    }

    /// 深度优先 处理 一个 配置文件, 返回其 file_id.
    ///
    /// `path` 必须是 归一化 后的 UNIX 形式 路径.
    fn load_file(&mut self, path: &str) -> Result<usize, Box<dyn Error>> {
        let key = norm_path(path);

        let text = self.r.read(&key)?;
        let file_id = self.src.add(key.clone(), text.clone());
        let conf = parse(&text, file_id)?;
        let includes = conf.include.clone();
        self.confs.push(conf);

        self.visiting.insert(key.clone());

        // 上级目录, 用于 解析 `include` 的 相对路径. 用 std::path 处理,
        // 输出前 转为 UNIX 形式.
        let dir = Path::new(&key).parent().unwrap_or_else(|| Path::new(""));
        for inc in &includes {
            let inc_path = dir.join(inc.as_ref());
            let inc_key = norm_path(&to_unix_path(inc_path.to_string_lossy().into_owned()));
            if self.loaded.contains(&inc_key) {
                continue;
            }
            if self.visiting.contains(&inc_key) {
                let mut p = HashMap::new();
                p.insert("file".to_string(), inc.as_ref().clone());
                return Err(Box::new(ConfError::IncludeCycle((
                    HasPD::new(inc.src(), format!("include cycle: `{}`", inc_key)),
                    p,
                ))));
            }
            self.load_file(&inc_key)?;
        }

        self.visiting.remove(&key);
        self.loaded.insert(key.clone());
        self.order.push(file_id);
        Ok(file_id)
    }
}

/// 加载全部 配置文件 (处理 `[[include]]`).
///
/// `src` 用于 记录 源文件信息, 加载失败时 也会保留 已读取的 源文件.
pub fn load<R: ReadConfFile>(
    path: &str,
    r: &mut R,
    src: &mut SrcInfo,
) -> Result<PmtcConf, Box<dyn Error>> {
    let mut l = Loader::new(r, src);
    let main_id = l.load_file(&norm_path(path))?;

    // eval_rec: 直接 使用 主配置文件 中的值, 否则 使用 默认值. 不检查 环境变量.
    // (库接口 不受 CLI 的 上限/PMTC_REC 限制; 由 CLI 在 调用 task::plan 前 处理.)
    let eval_rec = match &l.confs[main_id].eval_rec {
        Some(h) => *h.as_ref(),
        None => PMTC_DEFAULT_EVAL_REC,
    };

    // 忽略 其余 配置文件 中的 eval_rec (警告).
    for &file_id in &l.order {
        if file_id == main_id {
            continue;
        }
        if let Some(h) = &l.confs[file_id].eval_rec {
            let path = l.src.get(file_id).map(|(p, _)| p).unwrap_or_default();
            log::warn!(
                "eval_rec `{}` in `{}` ignored (only the main config file's eval_rec is used)",
                h.as_ref(),
                path
            );
        }
    }

    // env: 按 加载顺序 保留 每个 配置文件 的 [env].
    let mut env: Vec<PmtcEnvList> = Vec::new();
    // task: 同名 合并; 实例 按 加载顺序 (同一文件内按出现顺序).
    let mut task: HashMap<String, Vec<HasP<PmtcTask>>> = HashMap::new();
    // f: 无顺序; 后加载 覆盖 先加载 (警告).
    let mut f: HashMap<String, HasP<PmtcF>> = HashMap::new();

    for &file_id in &l.order {
        let conf = &l.confs[file_id];
        env.push(conf.env.clone());
        for (name, tasks) in &conf.task {
            task.entry(name.clone())
                .or_default()
                .extend(tasks.iter().cloned());
        }
        for (name, hf) in &conf.f {
            if f.contains_key(name) {
                log::warn!(
                    "duplicate [[f]] name `{}`: later config file overrides earlier",
                    name
                );
            }
            f.insert(name.clone(), hf.clone());
        }
    }

    // task/f 的 引用/依赖 检查.
    check_task_f(&task, &f)?;

    Ok(PmtcConf {
        eval_rec,
        env,
        task,
        f,
        src: l.src.clone(),
    })
}

#[cfg(test)]
mod test;
