//! 模板 (字符串 展开)
//!
//! # 递归深度 限制 (防止 死循环)
//!
//! `eval()`/`exec_eval()`/`conf_env()` 的 `r` 参数 限制 变量 展开的 嵌套深度,
//! 防止 特殊构造的 模板 (比如 变量 通过 `[n:NAME]` (n >= 2) 引用 自身/形成 环)
//! 导致 无限递归. 超出深度 时 返回 [`TeError::EvalRec`].
//!
//! `r` 的 来源:
//! - CLI: 由 `PMTC_REC` 环境变量 指定 (见 `cli` 模块), 最大接受 200.
//! - 库调用: 调用者 直接传入 `r`, 不受 200 限制, 由调用者 自行负责 取值.
//!
//! ## 注意 (库调用者)
//!
//! - `r` 限制的是 嵌套深度, 不是 总展开次数. 即使 引用图中 没有 环,
//!   多层 `[n:NAME]` 嵌套复制 (比如 `A="[2:B][2:B]"`, `B="[2:C][2:C]"`, ...)
//!   的 总展开数 是 指数级 的, 足够大的 `r` 下 仍会 耗尽 时间/内存.
//! - 过大的 `r` 会先 耗尽 真实调用栈 (栈溢出, 进程 abort),
//!   早于 深度检查 触发. 请使用 合理的 上限 (建议 不超过 几百).
//! - 注意 `r + 1` 的 溢出: 不要传入 `u16::MAX`.
use std::collections::{BTreeMap, HashMap, HashSet};

use icu_normalizer::ComposingNormalizerBorrowed;
use serde::Serialize;

use crate::conf::{HasP, SrcP};

mod error;
mod eval_impl;

pub use error::TeError;
pub use eval_impl::{eval, eval_use, exec_eval, exec_eval_2, exec_eval_use, exec_eval_use_2};

// 变量 标签 (内部 分类)

/// 默认标签: 配置文件 (`[env]`, `[[t.*]]`) 定义变量
pub const PMTC_ENV_LABEL_DEFAULT: u8 = 0;
/// 系统 (进程) 环境变量
pub const PMTC_ENV_LABEL_SYS: u8 = 1;
/// 从 `pm_tc.env` 文件中 加载/保存
pub const PMTC_ENV_LABEL_DOT: u8 = 2;
/// 内置变量 (比如 `date`)
pub const PMTC_ENV_LABEL_BUILTIN: u8 = 3;

/// 特殊变量 `HOME`: 用于 exec_eval 展开 `~` 路径
pub const PMTC_ENV_HOME: &'static str = "HOME";

/// 特殊字符: `\` 转义字符 (使用 \\ 转义 \ 自身)
pub const C_BS: char = '\\';
/// 特殊字符: `\n` 换行
pub const C_LF: char = '\n';
/// 特殊字符: `[` 模板 (替换/变量) 开始
pub const C_VS: char = '[';
/// 特殊字符: `]` 模板 (替换/变量) 结束
pub const C_VE: char = ']';
/// 特殊字符: `:` 用于 变量 展开 语法
pub const C_CO: char = ':';
/// 特殊字符: ` ` (空格) 不允许用于 变量名称
pub const C_SP: char = ' ';

/// 变量集
#[derive(Debug, Clone, Serialize)]
pub struct PmtcEnv {
    /// 内部保存的 变量 数据.
    ///
    /// `name` -> `(value, label)`
    var: HashMap<String, (String, u8)>,
    /// 使用的变量 (通过 `get_use` 标记).
    ///
    /// 只考虑本 `PmtcEnv.get_use()`, 如果把另一个 `PmtcEnv` 添加到自己, 不会处理 use 标记.
    ///
    /// `HashSet<(name, label)>`
    var_use: HashSet<(String, u8)>,
}

impl PmtcEnv {
    /// 空变量集
    pub fn new() -> Self {
        Self {
            var: HashMap::new(),
            var_use: HashSet::new(),
        }
    }

    /// 从一组值创建 (默认标签)
    pub fn from_map(map: &HashMap<String, String>) -> Self {
        Self::from_map_label(map, PMTC_ENV_LABEL_DEFAULT)
    }

    /// 从一组值创建 (带 标签)
    pub fn from_map_label(map: &HashMap<String, String>, label: u8) -> Self {
        let mut o = Self::new();
        o.add_map_label(map, label);
        o
    }

    /// 读取变量值 (不含 标签)
    pub fn get(&self, name: &str) -> Option<String> {
        self.get_label(name).map(|i| i.0)
    }

    /// 读取变量值 (`use` 标记).
    ///
    /// 即标记此变量 **被使用过**.
    pub fn get_use(&mut self, name: &str) -> Option<String> {
        self.get_use_label(name).map(|i| i.0)
    }

    /// 读取变量值 (带 标签).
    ///
    /// `返回 (value, label)`
    pub fn get_label(&self, name: &str) -> Option<(String, u8)> {
        self.var.get(name).map(|i| i.clone())
    }

    /// 读取变量值 (带 标签) (`use` 标记).
    pub fn get_use_label(&mut self, name: &str) -> Option<(String, u8)> {
        match self.get_label(name) {
            Some((value, label)) => {
                // 存储使用标记 (name, label)
                self.var_use.insert((name.to_string(), label));
                Some((value, label))
            }
            None => None,
        }
    }

    /// 设置变量值 (覆盖) (默认标签)
    pub fn set(&mut self, name: &str, value: &str) -> &mut Self {
        self.set_label(name, value, PMTC_ENV_LABEL_DEFAULT)
    }

    /// 设置变量值 (覆盖) (带 标签)
    pub fn set_label(&mut self, name: &str, value: &str, label: u8) -> &mut Self {
        self.var
            .insert(name.to_string(), (value.to_string(), label));
        self
    }

    /// 删除 变量
    pub fn rm(&mut self, name: &str) -> Option<(String, u8)> {
        self.var.remove(name)
    }

    /// 清空 `use` 标记
    pub fn use_clear(&mut self) {
        self.var_use.clear();
    }

    /// 获取 `use` 标记
    pub fn use_get(&self) -> HashSet<(String, u8)> {
        self.var_use.clone()
    }

    /// 添加一组值 (覆盖) (带 标签)
    pub fn add_map_label(&mut self, map: &HashMap<String, String>, label: u8) -> &mut Self {
        for (k, v) in map.iter() {
            self.set_label(k, v, label);
        }
        self
    }

    /// 添加 另一个 `PmtcEnv`
    pub fn add(&mut self, env: PmtcEnv) -> &mut Self {
        for (k, (v, label)) in env {
            self.set_label(&k, &v, label);
        }
        self
    }

    /// 转换类型 (不含 标签)
    pub fn to_map(&self) -> HashMap<String, String> {
        let mut o = HashMap::new();
        for (k, (v, _)) in self.clone() {
            o.insert(k, v);
        }
        o
    }

    /// 转换类型 (有序: 按 变量名 排序)
    pub fn to_map_sort(&self) -> BTreeMap<String, String> {
        self.to_map()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }
}

impl IntoIterator for PmtcEnv {
    type Item = (String, (String, u8));
    type IntoIter = std::collections::hash_map::IntoIter<String, (String, u8)>;

    fn into_iter(self) -> Self::IntoIter {
        self.var.into_iter()
    }
}

/// `[env]` 中 一个变量 的 操作.
///
/// 字符串 表示 定义/覆盖变量, 整数 `0` 表示 删除变量.
#[derive(Debug, Clone)]
pub enum EnvVar {
    /// 当前语法: 新值 覆盖 旧值.
    Set(String),
    /// 删除语法: 删除 同名 变量.
    Rm,
}

/// 待执行的 `[env]` 列表 (保留原 顺序)
#[derive(Debug, Clone)]
pub struct PmtcEnvList {
    /// (k, v)
    list: Vec<HasP<(String, EnvVar)>>,
}

impl PmtcEnvList {
    /// 空列表
    pub fn new() -> Self {
        Self { list: Vec::new() }
    }

    /// 添加 变量定义 (覆盖旧值)
    pub fn add(&mut self, k: &str, v: &str, src: SrcP) -> &mut Self {
        self.list
            .push(HasP::new(src, (k.to_string(), EnvVar::Set(v.to_string()))));
        self
    }

    /// 添加 变量删除
    pub fn add_rm(&mut self, k: &str, src: SrcP) -> &mut Self {
        self.list.push(HasP::new(src, (k.to_string(), EnvVar::Rm)));
        self
    }

    /// 转换类型
    pub fn to_vec(&self) -> Vec<(String, EnvVar)> {
        self.list.iter().map(|i| i.as_ref().clone()).collect()
    }
}

impl IntoIterator for PmtcEnvList {
    type Item = HasP<(String, EnvVar)>;
    type IntoIter = std::vec::IntoIter<HasP<(String, EnvVar)>>;

    fn into_iter(self) -> Self::IntoIter {
        self.list.into_iter()
    }
}

/// 模板 的一个 变量 部分
#[derive(Debug, Clone)]
pub struct TeVar {
    /// 变量 名称
    pub name: String,
    /// 展开次数.
    /// - 对于 `[NAME]` 语法, 展开 1 次.
    /// - 对于 `[:NAME]` 语法, 展开 0 次 (本次跳过).
    /// - 对于 `[n:NAME]` 语法 (n >= 2, 比如 `[2:NAME]`), 展开 n 次.
    pub count: u8,
}

impl TeVar {
    /// 构造 TeVar
    pub fn new(name: &str, count: u8) -> Self {
        Self {
            name: name.to_string(),
            count,
        }
    }
}

/// 模板 的一部分 (词法切分后)
#[derive(Debug, Clone)]
pub enum Part {
    /// 普通字符串, 透传
    S(String),
    /// 变量, eval 时 按规则 展开
    V(TeVar),
}

/// Unicode NFKC
pub fn unicode_nfkc(text: &str) -> String {
    let nfkc = ComposingNormalizerBorrowed::new_nfkc();
    nfkc.normalize(text).to_string()
}

/// 检查 变量名称 是否符合语法.
///
/// - (1) 变量名称 不得含有任何 特殊字符: C_BS, C_LF, C_VS, C_VE, C_CO, C_SP.
/// - (2) 变量名称 进行 NFKC 之后, 应该保持不变.
/// - (3) 变量名称 不允许为空.
///
/// pos 是 变量名称 的相对位置 (用于 生成错误).
/// 如果没有相对位置 (只是在检查 变量名称 自身, 不位于 模板 之中), 可以传 0.
pub fn check_name(name: &str, pos: usize) -> Result<String, TeError> {
    if name.is_empty() {
        return Err(TeError::NameEmpty(pos));
    }

    for (off, c) in name.chars().enumerate() {
        match c {
            C_BS | C_LF | C_VS | C_VE | C_CO | C_SP => {
                return Err(TeError::NameChar((pos + off, c)));
            }
            _ => {}
        }
    }

    let after = unicode_nfkc(name);
    if after != name {
        return Err(TeError::NameNFKC((pos, name.to_string(), after)));
    }

    Ok(after)
}

/// 对 模板 (字符串) 进行 语法解析.
///
/// 解析过程中, 转义序列 原样保留 (成为 `Part::S`), 从而 模板 即使多次执行, 也不会损失 转义序列.
/// 去除 转义 要等到 `end` 操作.
pub fn parse_t(t: &str) -> Result<Vec<Part>, TeError> {
    let chars: Vec<char> = t.chars().collect();
    let len = chars.len();
    let mut o: Vec<Part> = Vec::new();
    let mut s = String::new();
    let mut i = 0;

    while i < len {
        let c = chars[i];
        match c {
            // 转义序列 `\\`
            C_BS => match chars.get(i + 1) {
                Some(&C_BS) | Some(&C_VS) | Some(&C_VE) => {
                    s.push(C_BS);
                    s.push(chars[i + 1]);
                    i += 2;
                }
                Some(&c2) => return Err(TeError::Escape((i, Some(c2)))),
                None => return Err(TeError::Escape((i, None))),
            },
            // `[`
            C_VS => {
                // 寻找 匹配的 `]`
                let mut j = i + 1;
                while j < len && chars[j] != C_VE {
                    j += 1;
                }
                if j >= len {
                    return Err(TeError::VE((i, chars[i + 1..].iter().collect())));
                }

                let content: String = chars[i + 1..j].iter().collect();

                // 允许的形式: `[NAME]`, `[:NAME]`, `[n:NAME]` (n >= 2)
                let (count, name, name_off) = match content.find(C_CO) {
                    Some(k) => {
                        let (pre, name) = content.split_at(k);
                        let name = &name[C_CO.len_utf8()..];
                        if pre.is_empty() {
                            // `[:NAME]`
                            (0u8, name, k + C_CO.len_utf8())
                        } else {
                            // `[n:NAME]` (n >= 2)
                            match pre.parse::<u8>() {
                                Ok(n) if n >= 2 => (n, name, k + C_CO.len_utf8()),
                                _ => return Err(TeError::VF((i, content))),
                            }
                        }
                    }
                    // `[NAME]`
                    None => (1u8, content.as_str(), 0),
                };

                let name = check_name(name, i + 1 + name_off)?;

                if !s.is_empty() {
                    o.push(Part::S(std::mem::take(&mut s)));
                }
                o.push(Part::V(TeVar::new(&name, count)));
                i = j + 1;
            }
            _ => {
                s.push(c);
                i += 1;
            }
        }
    }

    if !s.is_empty() {
        o.push(Part::S(s));
    }
    Ok(o)
}

/// 检查 模板 (字符串) 是否符合语法
pub fn check_t(t: &str) -> Result<String, TeError> {
    let _ = parse_t(t)?;
    Ok(t.to_string())
}

/// 对 字符串 进行 最终求值 (去转义)
pub fn end(t: &str) -> Result<String, TeError> {
    let chars: Vec<char> = t.chars().collect();
    let len = chars.len();
    let mut o = String::new();
    let mut i = 0;

    while i < len {
        let c = chars[i];
        if c == C_BS {
            match chars.get(i + 1) {
                Some(&C_BS) | Some(&C_VS) | Some(&C_VE) => {
                    // 去除 转义
                    o.push(chars[i + 1]);
                    i += 2;
                }
                Some(&c2) => return Err(TeError::Escape((i, Some(c2)))),
                None => return Err(TeError::Escape((i, None))),
            }
        } else {
            o.push(c);
            i += 1;
        }
    }
    Ok(o)
}

/// 特殊处理: 缺失 `HOME` 变量时, 保持 `~` (不展开)
pub fn get_home(env: &PmtcEnv) -> String {
    env.get(PMTC_ENV_HOME).unwrap_or("~".to_string())
}

/// 展开 `~` 路径
pub fn path_home(path: String, home: &str) -> String {
    // 分别处理 `~/` (一般情况) 和 `~` (特殊情况)
    match path.strip_prefix("~/") {
        Some(p) => format!("{}/{}", &home, p),
        None => {
            if "~" == &path {
                home.to_string()
            } else {
                path
            }
        }
    }
}

/// 按顺序 执行 `[env]` (eval), 后定义的 变量 覆盖旧变量
///
/// 返回: (结果, 未知变量)
pub fn conf_env(
    env: &PmtcEnv,
    run: Vec<PmtcEnvList>,
    r: u16,
) -> Result<(PmtcEnv, HashSet<String>), HasP<TeError>> {
    let mut o = env.clone();
    // 未知变量
    let mut u = HashSet::new();

    for i in run {
        for p in i {
            let (name, v) = p.as_ref();
            match v {
                EnvVar::Set(value) => {
                    let value =
                        eval_use(value, &mut o, r, &mut u).map_err(|e| HasP::new(p.src(), e))?;
                    o.set(name, &value);
                }
                EnvVar::Rm => {
                    o.rm(name);
                }
            }
        }
    }
    Ok((o, u))
}

#[cfg(test)]
mod test;
