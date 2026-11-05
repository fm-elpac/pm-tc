//! 配置文件解析的 业务校验 实现.
//!
//! 解析分为 2 个阶段:
//! 1. 阶段 1: 使用 [`super::p1`] 把 `toml_edit::DocumentMut` 反序列化为
//!    带位置的 中间结构 (`P1`), 只检查 toml 内容是否 符合 抽象数据结构.
//! 2. 阶段 2: 在 阶段 1 结果的基础上, 转换数据 为 `PmtcConfFile`, 并做
//!    剩余的 错误检查 (`check_name`, `check_t`, 动作分派 等).
//!
//! 另外, 在 任何 数据处理 之前, 首先检查 配置文件 格式 版本号 (`pm_tc`).
use std::{
    collections::{HashMap, HashSet},
    error::Error,
};

use serde::{Deserialize, de::IntoDeserializer};
use toml_edit::{DocumentMut, Item, TomlError};

use crate::{
    check::te_err,
    te::{PmtcEnvList, TeError, check_name, check_t},
};

use super::{
    ConfError, HasP, HasPD, PMTC_CONF_VERSION, PmtcConfFile, PmtcF, PmtcFA, PmtcFChmod,
    PmtcFFileWrite, PmtcTask, SrcP,
    p1::{F1, P1, PathEval1, item_src, p1_de},
};

// ===== 错误 快捷生成 =====

/// conf 解析 的 错误类型
type CResult<T> = Result<T, ConfError>;

fn e_te(src: SrcP, e: TeError) -> ConfError {
    ConfError::Te(te_err(src, e))
}

// ===== 解析 辅助函数 =====

/// 检查 变量名称 (用于 `[env]`, `env:NAME`)
fn check_env_name(name: &str, src: SrcP) -> CResult<()> {
    check_name(name, 0).map_err(|e| e_te(src, e))?;
    Ok(())
}

/// 检查并去掉 `t.` 前缀 (task 引用).
///
/// 返回 去掉 `t.` 后的 名称.
fn strip_t(h: &HasP<String>) -> CResult<String> {
    let s = h.as_ref();
    match s.strip_prefix("t.") {
        Some(name) if !name.is_empty() => Ok(name.to_string()),
        _ => Err(ConfError::value(
            h.src(),
            "prefix",
            format!("task reference must start with `t.`: `{}`", s),
            &[("value", s.clone())],
        )),
    }
}

/// 检查并去掉 `f.` 前缀 (f 引用).
///
/// 返回 去掉 `f.` 后的 名称.
fn strip_f(h: &HasP<String>) -> CResult<String> {
    let s = h.as_ref();
    match s.strip_prefix("f.") {
        Some(name) if !name.is_empty() => Ok(name.to_string()),
        _ => Err(ConfError::value(
            h.src(),
            "prefix",
            format!("f reference must start with `f.`: `{}`", s),
            &[("value", s.clone())],
        )),
    }
}

/// 提取 必需的 字符串 参数 (动作 `a` 的 参数)
///
/// 缺少: `ConfError::FA`, 为空: `ConfError::FAEmpty`
fn require1(v: Option<HasP<String>>, src: SrcP, key: &str) -> CResult<HasP<String>> {
    match v {
        Some(i) => {
            if i.as_ref().is_empty() {
                Err(ConfError::fa_empty(
                    i.src(),
                    "empty",
                    format!("`{}` must not be empty", key),
                    &[("key", key.to_string())],
                ))
            } else {
                Ok(i)
            }
        }
        None => Err(ConfError::fa(
            src,
            "missing",
            format!("missing `{}`", key),
            &[("key", key.to_string())],
        )),
    }
}

/// 提取 必需的 字符串列表 参数 (动作 `a` 的 参数)
///
/// 缺少/为空: `ConfError::FA` / `ConfError::FAEmpty`
fn require_list(v: Option<Vec<HasP<String>>>, src: SrcP, key: &str) -> CResult<Vec<HasP<String>>> {
    match v {
        Some(l) => {
            if l.is_empty() {
                return Err(ConfError::fa_empty(
                    src,
                    "empty",
                    format!("`{}` must not be empty", key),
                    &[("key", key.to_string())],
                ));
            }
            for i in &l {
                if i.as_ref().is_empty() {
                    return Err(ConfError::fa_empty(
                        i.src(),
                        "element",
                        format!("`{}` element must not be empty", key),
                        &[("key", key.to_string())],
                    ));
                }
            }
            Ok(l)
        }
        None => Err(ConfError::fa(
            src,
            "missing",
            format!("missing `{}`", key),
            &[("key", key.to_string())],
        )),
    }
}

/// 检查 多余的 参数 (动作 `a` 不允许使用的 参数)
fn reject_extra(src: SrcP, a_name: &str, extra: &[(&str, bool)]) -> CResult<()> {
    for (k, b) in extra {
        if *b {
            return Err(ConfError::value(
                src,
                "extra",
                format!("`a = \"{}\"` does not allow parameter `{}`", a_name, k),
                &[("action", a_name.to_string()), ("param", (*k).to_string())],
            ));
        }
    }
    Ok(())
}

/// 检查 字符串模板 (如果存在).
fn opt_check_t(h: &Option<HasP<String>>) -> CResult<Option<HasP<String>>> {
    match h {
        Some(x) => Ok(Some(HasP::new(
            x.src(),
            check_t(x.as_ref()).map_err(|e| e_te(x.src(), e))?,
        ))),
        None => Ok(None),
    }
}

/// 检查 字符串模板 列表 (如果存在).
fn opt_check_t_list(h: &Option<Vec<HasP<String>>>) -> CResult<Option<Vec<HasP<String>>>> {
    match h {
        Some(l) => Ok(Some(
            l.iter()
                .map(|x| {
                    Ok(HasP::new(
                        x.src(),
                        check_t(x.as_ref()).map_err(|e| e_te(x.src(), e))?,
                    ))
                })
                .collect::<CResult<Vec<_>>>()?,
        )),
        None => Ok(None),
    }
}

/// 获取 整数 值 (使用 serde 反序列化)
fn get_int(item: &Item, src: SrcP, key: &str) -> CResult<i64> {
    let v = match item.as_value() {
        Some(i) => i,
        None => {
            return Err(ConfError::value(
                src,
                "int",
                format!("`{}` must be an integer", key),
                &[("key", key.to_string())],
            ));
        }
    };
    i64::deserialize(v.clone().into_deserializer()).map_err(|_| {
        ConfError::value(
            src,
            "int",
            format!("`{}` must be an integer", key),
            &[("key", key.to_string())],
        )
    })
}

// ===== 版本号 检查 =====

/// 检查 配置文件 格式 版本号 (`pm_tc`).
///
/// 必须在 任何 其他 数据处理 之前 调用.
fn check_version(doc: &DocumentMut, file_id: usize) -> CResult<()> {
    let item = match doc.get("pm_tc") {
        Some(i) => i,
        None => {
            return Err(ConfError::conf_version(
                SrcP::new(file_id, 0),
                "missing",
                "missing config format version `pm_tc`",
                &[],
            ));
        }
    };
    let src = item_src(item, file_id);
    let v = get_int(item, src, "pm_tc")?;
    if v != i64::from(PMTC_CONF_VERSION) {
        return Err(ConfError::conf_version(
            src,
            "unsupported",
            format!(
                "unsupported config format version: `{}` (supported: `{}`)",
                v, PMTC_CONF_VERSION
            ),
            &[
                ("version", v.to_string()),
                ("supported", PMTC_CONF_VERSION.to_string()),
            ],
        ));
    }
    Ok(())
}

// ===== 阶段 2: 中间结构 -> PmtcConfFile =====

/// `[[f]]` 定义 (中间结果)
#[derive(Debug, Clone)]
struct F2 {
    pub name: HasP<String>,
    pub once: HasP<u8>,
    pub skip_file_eval: Option<HasP<String>>,
    pub skip_include_eval: Option<HasP<String>>,
    pub dep: Option<Vec<HasP<String>>>,
    pub a: Option<HasP<String>>,
    pub mode: Option<HasP<String>>,
    pub exec_eval: Option<HasP<String>>,
    pub path_eval: Option<HasP<String>>,
    pub path_eval_a: Option<Vec<HasP<String>>>,
    pub file_eval: Option<HasP<String>>,
    pub text_eval: Option<HasP<String>>,
    pub log_eval: Option<Vec<HasP<String>>>,
}

fn p1_to_conf(p1: P1, file_id: usize) -> CResult<PmtcConfFile> {
    // `eval_rec` (None 表示 配置文件 中 没有 设置)
    let eval_rec = match &p1.eval_rec {
        Some(h) => {
            let v = u16::try_from(*h.as_ref()).map_err(|_| {
                ConfError::value(
                    h.src(),
                    "range",
                    format!("`eval_rec` out of range (0 ~ {}): {}", u16::MAX, h.as_ref()),
                    &[
                        ("key", "eval_rec".to_string()),
                        ("value", h.as_ref().to_string()),
                    ],
                )
            })?;
            Some(HasP::new(h.src(), v))
        }
        None => None,
    };

    // `[[include]]`: 只允许 `file`, 缺少 `file` 报错, 去重.
    let mut include = Vec::new();
    for inc in &p1.include {
        match &inc.file {
            Some(f) => include.push(HasP::new(f.src(), f.as_ref().clone())),
            None => {
                return Err(ConfError::value(
                    SrcP::new(file_id, 0),
                    "missing",
                    "`[[include]]` missing `file`",
                    &[("key", "file".to_string())],
                ));
            }
        }
    }
    let mut seen: HashSet<String> = HashSet::new();
    for i in &include {
        if !seen.insert(i.as_ref().clone()) {
            return Err(ConfError::include_dup(
                i.src(),
                "dup",
                format!("duplicate `[[include]]` in the same file: `{}`", i.as_ref()),
                &[("file", i.as_ref().clone())],
            ));
        }
    }

    // `[env]`: 检查 变量名称, 模板语法 (保持 顺序).
    let mut env = PmtcEnvList::new();
    for (name, ks, value) in &p1.env.list {
        check_env_name(name, *ks)?;
        check_t(value.as_ref()).map_err(|e| e_te(value.src(), e))?;
        env.add(name, value.as_ref(), value.src());
    }

    // `[[t.*]]`: 检查 允许的 key, 变量名称, 模板语法.
    let mut task: HashMap<String, Vec<HasP<PmtcTask>>> = HashMap::new();
    for (name, tasks) in &p1.t {
        for ht in tasks {
            let src = ht.src();
            let t = ht.as_ref();
            let mut out = PmtcTask {
                _name: format!("t.{}", name),
                env: HashMap::new(),
                env_for: None,
                f: None,
                dep: Vec::new(),
                arg: HasP::new(src, 0),
            };
            if let Some(h) = &t.env_for {
                out.env_for = Some(HasP::new(h.src(), strip_t(h)?));
            }
            if let Some(h) = &t.f {
                // `f` 字段 已经是 f 名称, 直接使用 (不带 `f.` 前缀).
                out.f = Some(HasP::new(h.src(), h.as_ref().clone()));
            }
            out.dep = t
                .dep
                .iter()
                .map(|h| Ok(HasP::new(h.src(), strip_t(h)?)))
                .collect::<CResult<Vec<_>>>()?;
            if let Some(h) = &t.arg {
                let n = usize::try_from(*h.as_ref()).map_err(|_| {
                    ConfError::value(
                        h.src(),
                        "range",
                        format!("`arg` out of range: {}", h.as_ref()),
                        &[
                            ("key", "arg".to_string()),
                            ("value", h.as_ref().to_string()),
                        ],
                    )
                })?;
                out.arg = HasP::new(h.src(), n);
            }
            for (ename, (ks, value)) in &t.env {
                check_env_name(ename, *ks)?;
                check_t(value.as_ref()).map_err(|e| e_te(*ks, e))?;
                out.env
                    .insert(ename.clone(), HasP::new(*ks, value.as_ref().clone()));
            }
            task.entry(name.clone())
                .or_default()
                .push(HasP::new(src, out));
        }
    }

    // `[[f]]`: 检查 允许的 key, 类型, `once` 取值, 模板语法, name 去重, 动作分派.
    let mut f: HashMap<String, HasP<PmtcF>> = HashMap::new();
    for hf in &p1.f {
        let f2 = f1_to_f2(hf.as_ref(), file_id)?;
        let name = f2.name.as_ref().clone();
        if f.contains_key(&name) {
            return Err(ConfError::value(
                f2.name.src(),
                "dup",
                format!("duplicate `[[f]]` in the same file: `{}`", name),
                &[("name", name.clone())],
            ));
        }
        let pf = f2_to_f(f2)?;
        f.insert(name, pf);
    }

    Ok(PmtcConfFile {
        pm_tc: PMTC_CONF_VERSION,
        eval_rec,
        include,
        env,
        task,
        f,
    })
}

/// `[[f]]` 中间结构 (`F1`) -> `F2`.
fn f1_to_f2(f1: &F1, file_id: usize) -> CResult<F2> {
    let name = f1
        .name
        .clone()
        .unwrap_or_else(|| HasP::new(SrcP::new(file_id, 0), String::new()));
    let once = match &f1.once {
        Some(h) => {
            let n = *h.as_ref();
            if n > 1 {
                return Err(ConfError::value(
                    h.src(),
                    "range",
                    format!("`once` must be 0 or 1: {}", n),
                    &[("key", "once".to_string()), ("value", n.to_string())],
                ));
            }
            HasP::new(h.src(), n as u8)
        }
        None => HasP::new(SrcP::new(file_id, 0), 0),
    };
    let skip_file_eval = opt_check_t(&f1.skip_file_eval)?;
    let skip_include_eval = opt_check_t(&f1.skip_include_eval)?;
    let dep = if f1.dep.is_empty() {
        None
    } else {
        Some(
            f1.dep
                .iter()
                .map(|h| Ok(HasP::new(h.src(), strip_f(h)?)))
                .collect::<CResult<Vec<_>>>()?,
        )
    };
    let a = f1.a.clone();
    let mode = f1.mode.clone();
    let exec_eval = opt_check_t(&f1.exec_eval)?;
    let path_eval = match &f1.path_eval {
        Some(PathEval1::S(h)) => Some(h.clone()),
        _ => None,
    };
    let path_eval_a = match &f1.path_eval {
        Some(PathEval1::A(l)) => Some(l.clone()),
        _ => f1.path_eval_a.clone(),
    };
    let file_eval = opt_check_t(&f1.file_eval)?;
    let text_eval = opt_check_t(&f1.text_eval)?;
    let log_eval = opt_check_t_list(&f1.log_eval)?;

    Ok(F2 {
        name,
        once,
        skip_file_eval,
        skip_include_eval,
        dep,
        a,
        mode,
        exec_eval,
        path_eval,
        path_eval_a,
        file_eval,
        text_eval,
        log_eval,
    })
}

/// `[[f]]` 动作 `a` 分派, 参数检查 (`F2` -> `HasP<PmtcF>`).
fn f2_to_f(f2: F2) -> CResult<HasP<PmtcF>> {
    let fsrc = f2.name.src();
    let a_src = f2.a.as_ref().map(|i| i.src()).unwrap_or(fsrc);

    // 合并 `path_eval` (字符串) 和 `path_eval_a` (字符串数组)
    let paths: Option<Vec<HasP<String>>> = match (&f2.path_eval, &f2.path_eval_a) {
        (Some(_), Some(_)) => {
            return Err(ConfError::value(
                fsrc,
                "path",
                "`path_eval` and `path_eval_a` cannot both be used",
                &[],
            ));
        }
        (Some(s), None) => Some(vec![HasP::new(s.src(), s.as_ref().clone())]),
        (None, Some(l)) => Some(l.clone()),
        (None, None) => None,
    };
    let path_is_some = paths.is_some();
    let mode_is_some = f2.mode.is_some();
    let exec_is_some = f2.exec_eval.is_some();
    let file_is_some = f2.file_eval.is_some();
    let text_is_some = f2.text_eval.is_some();
    let log_is_some = f2.log_eval.is_some();

    let action = match f2.a.as_ref().map(|i| i.as_ref().as_str()) {
        None => {
            if exec_is_some
                || path_is_some
                || file_is_some
                || text_is_some
                || log_is_some
                || mode_is_some
            {
                return Err(ConfError::fa(
                    fsrc,
                    "missing",
                    "missing `a` (action type); cannot use action parameters",
                    &[("key", "a".to_string())],
                ));
            }
            None
        }
        Some("exec") => {
            reject_extra(
                fsrc,
                "exec",
                &[
                    ("path_eval", path_is_some),
                    ("file_eval", file_is_some),
                    ("text_eval", text_is_some),
                    ("log_eval", log_is_some),
                    ("mod", mode_is_some),
                ],
            )?;
            Some(PmtcFA::Exec(require1(
                f2.exec_eval.clone(),
                a_src,
                "exec_eval",
            )?))
        }
        Some("exec_sh") => {
            reject_extra(
                fsrc,
                "exec_sh",
                &[
                    ("path_eval", path_is_some),
                    ("file_eval", file_is_some),
                    ("text_eval", text_is_some),
                    ("log_eval", log_is_some),
                    ("mod", mode_is_some),
                ],
            )?;
            Some(PmtcFA::ExecSh(require1(
                f2.exec_eval.clone(),
                a_src,
                "exec_eval",
            )?))
        }
        Some("echo") => {
            reject_extra(
                fsrc,
                "echo",
                &[
                    ("path_eval", path_is_some),
                    ("exec_eval", exec_is_some),
                    ("file_eval", file_is_some),
                    ("log_eval", log_is_some),
                    ("mod", mode_is_some),
                ],
            )?;
            Some(PmtcFA::Echo(require1(
                f2.text_eval.clone(),
                a_src,
                "text_eval",
            )?))
        }
        Some("log") => {
            reject_extra(
                fsrc,
                "log",
                &[
                    ("path_eval", path_is_some),
                    ("exec_eval", exec_is_some),
                    ("file_eval", file_is_some),
                    ("text_eval", text_is_some),
                    ("mod", mode_is_some),
                ],
            )?;
            Some(PmtcFA::Log(require_list(
                f2.log_eval.clone(),
                a_src,
                "log_eval",
            )?))
        }
        Some("cat") => {
            reject_extra(
                fsrc,
                "cat",
                &[
                    ("path_eval", path_is_some),
                    ("exec_eval", exec_is_some),
                    ("text_eval", text_is_some),
                    ("log_eval", log_is_some),
                    ("mod", mode_is_some),
                ],
            )?;
            Some(PmtcFA::Cat(require1(
                f2.file_eval.clone(),
                a_src,
                "file_eval",
            )?))
        }
        Some("mkdir") => {
            reject_extra(
                fsrc,
                "mkdir",
                &[
                    ("exec_eval", exec_is_some),
                    ("file_eval", file_is_some),
                    ("text_eval", text_is_some),
                    ("log_eval", log_is_some),
                    ("mod", mode_is_some),
                ],
            )?;
            let p = require_list(paths, a_src, "path_eval")?;
            if p.len() != 1 {
                return Err(ConfError::value(
                    fsrc,
                    "mkdir",
                    "`mkdir` supports a single `path_eval` only",
                    &[],
                ));
            }
            let mut p = p;
            Some(PmtcFA::Mkdir(p.swap_remove(0)))
        }
        Some("chmod") => {
            reject_extra(
                fsrc,
                "chmod",
                &[
                    ("exec_eval", exec_is_some),
                    ("file_eval", file_is_some),
                    ("text_eval", text_is_some),
                    ("log_eval", log_is_some),
                ],
            )?;
            let m = match &f2.mode {
                Some(i) => i,
                None => {
                    return Err(ConfError::fa(
                        a_src,
                        "missing",
                        "`chmod` missing `mod`",
                        &[("key", "mod".to_string())],
                    ));
                }
            };
            let m_u32 = u32::from_str_radix(m.as_ref(), 8).map_err(|_| {
                ConfError::value(
                    m.src(),
                    "octal",
                    format!(
                        "`mod` must be an octal number (e.g. `700`): `{}`",
                        m.as_ref()
                    ),
                    &[("key", "mod".to_string()), ("value", m.as_ref().clone())],
                )
            })?;
            let p = require_list(paths, a_src, "path_eval")?;
            Some(PmtcFA::Chmod(PmtcFChmod {
                mode: HasP::new(m.src(), m_u32),
                path_eval: p,
            }))
        }
        Some("file") | Some("file_append") | Some("file_merge") => {
            let a_name =
                f2.a.as_ref()
                    .map(|i| i.as_ref().clone())
                    .unwrap_or_default();
            reject_extra(
                fsrc,
                &a_name,
                &[
                    ("path_eval", path_is_some),
                    ("exec_eval", exec_is_some),
                    ("log_eval", log_is_some),
                    ("mod", mode_is_some),
                ],
            )?;
            let w = PmtcFFileWrite {
                file_eval: require1(f2.file_eval.clone(), a_src, "file_eval")?,
                text_eval: require1(f2.text_eval.clone(), a_src, "text_eval")?,
            };
            Some(match a_name.as_str() {
                "file" => PmtcFA::File(w),
                "file_append" => PmtcFA::FileAppend(w),
                _ => PmtcFA::FileMerge(w),
            })
        }
        Some(other) => {
            return Err(ConfError::fa(
                a_src,
                "unknown",
                format!("unknown action type `a`: `{}`", other),
                &[("action", other.to_string())],
            ));
        }
    };

    let pf = PmtcF {
        _name: f2.name.as_ref().clone(),
        once: *f2.once.as_ref() != 0,
        skip_file_eval: f2.skip_file_eval.clone(),
        skip_include_eval: f2.skip_include_eval.clone(),
        dep: f2.dep.clone().unwrap_or_default(),
        a: action.map(|x| HasP::new(a_src, x)),
    };
    Ok(HasP::new(fsrc, pf))
}

// ===== 解析 =====

/// 解析 单个 配置文件.
///
/// file_id: 用于生成错误.
pub fn parse(src: &str, file_id: usize) -> Result<PmtcConfFile, Box<dyn Error>> {
    let doc: DocumentMut = src.parse().map_err(|e: TomlError| {
        let p = e.span().map(|s| s.start as isize).unwrap_or(0);
        ConfError::Toml(HasPD::new(SrcP::new(file_id, p), e))
    })?;

    // 首先检查 版本号 (在 任何 其他 数据处理 之前)
    check_version(&doc, file_id)?;

    let p1 = p1_de::<P1>(&doc, file_id)?;
    let conf = p1_to_conf(p1, file_id)?;
    Ok(conf)
}
