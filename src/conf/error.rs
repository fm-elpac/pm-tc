//! 错误处理
use std::{
    collections::HashMap,
    error::Error,
    fmt::{self, Debug},
};

use toml_edit::TomlError;

use crate::te::TeError;

/// 源文件位置 (用于调试/报错)
#[derive(Debug, Clone, Copy, Default)]
pub struct SrcP {
    /// .toml 文件 (序号)
    pub file_id: usize,
    /// .toml 文件内的位置 (toml_edit)
    pub i: isize,
}

impl SrcP {
    /// 构造 SrcP
    pub fn new(file_id: usize, i: isize) -> Self {
        Self { file_id, i }
    }
}

/// 源文件 信息 (用于 调试/报错)
/// 不检查 文件路径 是否重复
#[derive(Debug, Clone)]
pub struct SrcInfo {
    /// 文件名 (路径) 列表, 下标 (index) 为 file_id
    path: Vec<String>,

    /// 文件内位置 -> 行号 的映射
    /// line[file_id][line_n]
    line: Vec<Vec<usize>>,

    /// 存储完整的 源文件 内容
    /// file_id -> 文件内容
    file: Vec<String>,
}

impl SrcInfo {
    /// 空 SrcInfo
    pub fn new() -> Self {
        Self {
            path: Vec::new(),
            line: Vec::new(),
            file: Vec::new(),
        }
    }

    /// 添加一个新的 源文件.
    ///
    /// 返回 file_id
    pub fn add(&mut self, path: String, text: String) -> usize {
        let file_id = self.path.len();
        // assert: self.path.len() == self.line.len() == self.file.len()

        self.path.push(path);
        self.line.push(parse_line(&text));
        self.file.push(text);

        file_id
    }

    /// 检查 file_id 是否有效 (下标 是否超出)
    fn check_file_id(&self, file_id: usize) -> Result<usize, ConfError> {
        if (file_id < self.path.len()) && (file_id < self.line.len()) && (file_id < self.file.len())
        {
            return Ok(file_id);
        }
        Err(ConfError::FileId(file_id))
    }

    /// 获取 源文件.
    ///
    /// 返回 (path, text)
    pub fn get(&self, file_id: usize) -> Result<(String, String), ConfError> {
        self.check_file_id(file_id)
            .map(|i| (self.path[i].clone(), self.file[i].clone()))
    }

    /// 获取 文件名 (路径), 行号
    pub fn info(&self, src: &SrcP) -> Result<(String, usize), ConfError> {
        self.check_file_id(src.file_id).map(|i| {
            let path = self.path[i].clone();

            // 位置 (偏移) -> 行号
            // 行号 = 位于该偏移之前的 `\n` 数量 + 1
            let p = src.i.max(0) as usize;
            let line = self.line[i].partition_point(|&n| n < p) + 1;

            (path, line)
        })
    }
}

/// 计算文件中各行的偏移 (`\n` 出现位置)
fn parse_line(text: &str) -> Vec<usize> {
    text.match_indices('\n').map(|(i, _)| i).collect()
}

/// 带有 `SrcP` 信息的数据
#[derive(Debug, Clone)]
pub struct HasP<T: Debug + Clone + ?Sized> {
    s: SrcP,
    v: T,
}

impl<T: Debug + Clone + ?Sized> HasP<T> {
    /// 构造 HasP
    pub fn new(src: SrcP, value: T) -> Self {
        Self { s: src, v: value }
    }

    /// 获取 `SrcP`
    pub fn src(&self) -> SrcP {
        self.s.clone()
    }
}

impl<T: Debug + Clone + ?Sized> AsRef<T> for HasP<T> {
    fn as_ref(&self) -> &T {
        &self.v
    }
}

impl<T: Debug + Clone + ?Sized> AsMut<T> for HasP<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.v
    }
}

/// 带有 `SrcP` 信息的数据, 且可以显示
#[derive(Debug, Clone)]
pub struct HasPD<T: Debug + Clone + ?Sized + fmt::Display> {
    s: SrcP,
    v: T,
}

impl<T: Debug + Clone + ?Sized + fmt::Display> HasPD<T> {
    /// 构造 HasPD
    pub fn new(src: SrcP, value: T) -> Self {
        Self { s: src, v: value }
    }

    /// 获取 `SrcP`
    pub fn src(&self) -> SrcP {
        self.s.clone()
    }
}

impl<T: Debug + Clone + ?Sized + fmt::Display> AsRef<T> for HasPD<T> {
    fn as_ref(&self) -> &T {
        &self.v
    }
}

impl<T: Debug + Clone + ?Sized + fmt::Display> AsMut<T> for HasPD<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.v
    }
}

/// `HasPD<>` 默认显示格式
impl<T: Debug + Clone + ?Sized + fmt::Display> fmt::Display for HasPD<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.src();
        write!(f, "({}:{})  {}", s.file_id, s.i, self.as_ref())
    }
}

/// conf 错误.
///
/// 其中 `SrcP` (`HasPD`) 是 错误位置, 需配合 `SrcInfo` 获取用于显示的信息 (文件名, 行号).
/// 如果是 整个文件错误 (比如 toml 无法解析), 或者不方便获取具体位置,
/// `SrcP.i` 可以传 0, 不必使用 精确位置.
/// 也就是说, `SrcP.i` 只是 "尽力而为".
///
/// `why: String` 是 库 内置的 (默认) 简洁 (英文) 错误说明.
/// 同时附带 `HashMap<String, String>`, 用于保存关键错误信息的 key-value 值,
/// 用于 中文翻译/友好错误显示.
///
/// 注意: 此处的 `HashMap<String, String>` 中用的 key, 不计入 库 的稳定 API.
/// 友好错误信息 只是尽力而为.
#[derive(Debug, Clone)]
pub enum ConfError {
    /// `file_id` 无效
    FileId(usize),
    /// `*.toml` 文件解析错误 (toml_edit)
    Toml(HasPD<TomlError>),
    /// 不支持的 配置文件 格式 版本号.
    ///
    /// `HasPD<why>`: 错误情况说明.
    ConfVersion((HasPD<String>, HashMap<String, String>)),
    /// te 错误 (变量名 / 模板 错误)
    Te(HasPD<TeError>),
    /// 未知的 toml key (比如 随意定义的 `[XXX]`, `[[XXX]]`, `A = B`).
    ///
    /// `HasPD<why>`: 错误情况说明.
    Key((HasPD<String>, HashMap<String, String>)),
    /// (通用错误) toml key 的内容 (类型, 值 等) 不符合语法.
    ///
    /// `HasPD<why>`: 错误情况说明.
    Value((HasPD<String>, HashMap<String, String>)),
    /// 在同一个 `.toml` 文件中, `[[include]]` 相同的文件 (path 相同).
    ///
    /// `HasPD<why>`: 错误情况说明.
    IncludeDup((HasPD<String>, HashMap<String, String>)),
    /// (多个文件) 循环 `[[include]]`.
    ///
    /// `HasPD<why>`: 错误情况说明.
    IncludeCycle((HasPD<String>, HashMap<String, String>)),
    /// 循环依赖: `[[t.*]]`, `[[f]]` 的 `dep`
    ///
    /// `HasPD<why>`: 错误情况说明.
    DepCycle((HasPD<String>, HashMap<String, String>)),
    /// `[[f]]` 的 `a` 类型, 缺少必需的参数.
    ///
    /// `HasPD<why>`: 错误情况说明.
    FA((HasPD<String>, HashMap<String, String>)),
    /// `[[f]]` 的 `a` 类型, 相应参数 不能为空.
    ///
    /// `HasPD<why>`: 错误情况说明.
    FAEmpty((HasPD<String>, HashMap<String, String>)),
    /// (多个文件) 依赖 `dep` 引用的 `[[t.*]]` 不存在.
    ///
    /// `HasPD<why>`: 错误情况说明.
    DepT((HasPD<String>, HashMap<String, String>)),
    /// (多个文件) 依赖 `dep` 引用的 `[[f]]` 不存在.
    ///
    /// `HasPD<why>`: 错误情况说明.
    DepF((HasPD<String>, HashMap<String, String>)),
    /// (多个文件) `[[t.*]]` 引用的 `[[f]]` 不存在.
    ///
    /// `HasPD<why>`: 错误情况说明.
    F((HasPD<String>, HashMap<String, String>)),
    /// `[[f]]` (名称) 重复.
    ///
    /// `HasPD<why>`: 错误情况说明.
    FDup((HasPD<String>, HashMap<String, String>)),
}

/// 从 key-value 列表构造 错误 参数 (用于 中文翻译/友好显示).
fn params(ps: &[(&str, String)]) -> HashMap<String, String> {
    ps.iter()
        .map(|(k, v)| ((*k).to_string(), v.clone()))
        .collect()
}

/// 构造带 `t` 标记的错误参数.
///
/// `t` 用于区分复用同一 `ConfError` 变体 的不同情况, 供友好错误显示使用.
fn params_t(t: &str, ps: &[(&str, String)]) -> HashMap<String, String> {
    let mut m = params(ps);
    m.insert("t".to_string(), t.to_string());
    m
}

impl ConfError {
    pub(crate) fn value(src: SrcP, t: &str, why: impl Into<String>, ps: &[(&str, String)]) -> Self {
        Self::Value((HasPD::new(src, why.into()), params_t(t, ps)))
    }

    pub(crate) fn key(src: SrcP, t: &str, why: impl Into<String>, ps: &[(&str, String)]) -> Self {
        Self::Key((HasPD::new(src, why.into()), params_t(t, ps)))
    }

    pub(crate) fn conf_version(
        src: SrcP,
        t: &str,
        why: impl Into<String>,
        ps: &[(&str, String)],
    ) -> Self {
        Self::ConfVersion((HasPD::new(src, why.into()), params_t(t, ps)))
    }

    pub(crate) fn include_dup(
        src: SrcP,
        t: &str,
        why: impl Into<String>,
        ps: &[(&str, String)],
    ) -> Self {
        Self::IncludeDup((HasPD::new(src, why.into()), params_t(t, ps)))
    }

    pub(crate) fn fa(src: SrcP, t: &str, why: impl Into<String>, ps: &[(&str, String)]) -> Self {
        Self::FA((HasPD::new(src, why.into()), params_t(t, ps)))
    }

    pub(crate) fn fa_empty(
        src: SrcP,
        t: &str,
        why: impl Into<String>,
        ps: &[(&str, String)],
    ) -> Self {
        Self::FAEmpty((HasPD::new(src, why.into()), params_t(t, ps)))
    }
}

impl Error for ConfError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Toml(p) => Some(p.as_ref()),
            Self::Te(p) => Some(p.as_ref()),
            _ => None,
        }
    }
}

/// 此处的显示方式只是 缺省/默认 显示, 适用于 开发/调试.
/// 调用者 可以使用对用户更友好的显示方式.
impl fmt::Display for ConfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FileId(file_id) => write!(f, "ConfError::FileId {}", file_id),
            Self::Toml(p) => {
                let s = p.src();
                write!(
                    f,
                    "ConfError::Toml ({}:{})  {}",
                    s.file_id,
                    s.i,
                    p.as_ref().message()
                )
            }
            Self::ConfVersion(p) => write!(f, "ConfError::ConfVersion {}", p.0),
            Self::Te(p) => write!(f, "ConfError::Te {}", p),
            Self::Key(p) => write!(f, "ConfError::Key {}", p.0),
            Self::Value(p) => write!(f, "ConfError::Value {}", p.0),
            Self::IncludeDup(p) => write!(f, "ConfError::IncludeDup {}", p.0),
            Self::IncludeCycle(p) => write!(f, "ConfError::IncludeCycle {}", p.0),
            Self::DepCycle(p) => write!(f, "ConfError::DepCycle {}", p.0),
            Self::FA(p) => write!(f, "ConfError::FA {}", p.0),
            Self::FAEmpty(p) => write!(f, "ConfError::FAEmpty {}", p.0),
            Self::DepT(p) => write!(f, "ConfError::DepT {}", p.0),
            Self::DepF(p) => write!(f, "ConfError::DepF {}", p.0),
            Self::F(p) => write!(f, "ConfError::F {}", p.0),
            Self::FDup(p) => write!(f, "ConfError::FDup {}", p.0),
        }
    }
}
