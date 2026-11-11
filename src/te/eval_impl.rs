//! `eval` 功能 内部实现
use std::collections::HashSet;

use super::{Part, PmtcEnv, end, error::TeError, get_home, parse_t, path_home};

use log::{debug, trace};

/// 对 `PmtcEnv.get()` 和 `PmtcEnv.get_use()` 的统一抽象
trait EnvUse {
    fn get_use(&mut self, name: &str) -> Option<String>;
    fn as_ref(&self) -> &PmtcEnv;
}

/// 用于 `get_use()`
struct PmtcEnvUse<'a> {
    env: &'a mut PmtcEnv,
}

impl<'a> PmtcEnvUse<'a> {
    pub fn new(env: &'a mut PmtcEnv) -> Self {
        Self { env }
    }
}

impl<'a> EnvUse for PmtcEnvUse<'a> {
    fn get_use(&mut self, name: &str) -> Option<String> {
        self.env.get_use(name)
    }

    fn as_ref(&self) -> &PmtcEnv {
        &self.env
    }
}

/// 用于 `get()`
struct PmtcEnvNoUse<'a> {
    env: &'a PmtcEnv,
}

impl<'a> PmtcEnvNoUse<'a> {
    pub fn new(env: &'a PmtcEnv) -> Self {
        Self { env }
    }
}

impl<'a> EnvUse for PmtcEnvNoUse<'a> {
    fn get_use(&mut self, name: &str) -> Option<String> {
        self.env.get(name)
    }

    fn as_ref(&self) -> &PmtcEnv {
        self.env
    }
}

/// eval 的 实际实现 (处理 递归调用/死循环)
///
/// - `r`: 允许的 最大递归次数
/// - `s`: 递归调用栈 (用于 错误信息), 捕捉每次的 模板 `t`
/// - `u`: 未知变量
///
/// 返回 `(result, s)`
fn eval_r<T: EnvUse>(
    t: &str,
    env: &mut T,
    r: u16,
    mut s: Vec<String>,
    u: &mut HashSet<String>,
) -> Result<(String, Vec<String>), TeError> {
    // 首先存入 模板
    s.push(t.to_string());

    trace!("eval_r: ({}) {:?}", r, &s);
    // 检查 递归调用深度
    if s.len() > r.into() {
        // 报错时, 要恢复原始的 r
        return Err(TeError::EvalRec((r - 1, s)));
    }

    let p = parse_t(t)?;
    let mut o: Vec<String> = Vec::new();

    for i in p {
        let j = match i {
            Part::S(s) => s.to_string(),
            Part::V(v) => {
                if 0 == v.count {
                    // `[:NAME]`
                    // 本次不展开
                    format!("[{}]", v.name)
                } else {
                    // `[NAME]`
                    // 本次展开.
                    // 不存在的 变量, 替换为 "" (空), 记录到 未知变量.
                    let mut t = env.get_use(&v.name).unwrap_or_else(|| {
                        u.insert(v.name.clone());
                        "".to_string()
                    });

                    // `[n:NAME]` n >= 2
                    // 继续展开 n - 1 次
                    // (如果 1 == n, 则下面循环不执行, 从而统一 `[NAME]` 和 `[n:NAME]` 的执行)
                    for _ in 0..(v.count - 1) {
                        // 递归调用
                        (t, s) = eval_r(&t, env, r, s, u)?;
                    }
                    t
                }
            }
        };
        o.push(j);
    }
    // 取出 模板
    s.pop();
    Ok((o.join(""), s))
}

/// `eval` / `eval_use` 实际实现
fn eval_i<T: EnvUse>(
    t: &str,
    env: &mut T,
    r: u16,
    u: &mut HashSet<String>,
) -> Result<String, TeError> {
    // 递归调用栈
    let s: Vec<String> = Vec::new();

    // 当输入 r = 0 时, 允许 本次展开, 但不允许 递归, 所以有 r + 1.
    // 报错时, 记得恢复 原始 r. (r - 1)
    let (result, _) = eval_r(t, env, r + 1, s, u)?;
    Ok(result)
}

/// `exec_eval` / `exec_eval_use` 实际实现 (第 2 阶段)
fn exec_eval_2i<T: EnvUse>(
    t: String,
    mut env: T,
    r: u16,
    u: &mut HashSet<String>,
) -> Result<Vec<String>, TeError> {
    let a: Vec<String> = t.split_ascii_whitespace().map(|i| i.to_string()).collect();
    let mut o: Vec<String> = Vec::new();

    let home = get_home(env.as_ref());

    for i in a {
        let i = eval_i(&i, &mut env, r, u)?;
        let i = end(&i)?;
        let i = path_home(i, &home);
        o.push(i);
    }
    Ok(o)
}

/// `exec_eval` / `exec_eval_use` 实际实现
fn exec_eval_i<T: EnvUse>(
    t: &str,
    mut env: T,
    r: u16,
    u: &mut HashSet<String>,
) -> Result<Vec<String>, TeError> {
    let c = eval_i(t, &mut env, r, u)?;
    exec_eval_2i(c, env, r, u)
}

/// 对 模板 进行 单步展开 (不进行 end 处理).
///
/// r: 允许的 最大递归次数 (防止 死循环)
pub fn eval(t: &str, env: &PmtcEnv, r: u16) -> Result<String, TeError> {
    debug!("eval: ({}) {}", r, t);

    let mut env = PmtcEnvNoUse::new(env);
    let mut u = HashSet::new();
    eval_i(t, &mut env, r, &mut u)
}

/// 类似 `eval` 但是会标记 变量使用 (`use`).
///
/// - u: 未知变量
pub fn eval_use(
    t: &str,
    env: &mut PmtcEnv,
    r: u16,
    u: &mut HashSet<String>,
) -> Result<String, TeError> {
    debug!("eval_use: ({}) {}", r, t);

    let mut env = PmtcEnvUse::new(env);
    eval_i(t, &mut env, r, u)
}

/// 对 命令行 进行处理: 首先 eval 整体, 然后 空格 切分, 然后 eval/end 每个部分 (参数)
/// 并处理 `~` 路径展开 (`HOME` 变量)
pub fn exec_eval(t: &str, env: &PmtcEnv, r: u16) -> Result<Vec<String>, TeError> {
    let env = PmtcEnvNoUse::new(env);
    let mut u = HashSet::new();
    exec_eval_i(t, env, r, &mut u)
}

/// 类似 `exec_eval` 但只有 第 2 阶段 (即 不首先 eval 整个 字符串)
pub fn exec_eval_2(t: &str, env: &PmtcEnv, r: u16) -> Result<Vec<String>, TeError> {
    let env = PmtcEnvNoUse::new(env);
    let mut u = HashSet::new();
    exec_eval_2i(t.to_string(), env, r, &mut u)
}

/// 类似 `exec_eval` 但是会标记 变量使用 (`use`).
///
/// - u: 未知变量
pub fn exec_eval_use(
    t: &str,
    env: &mut PmtcEnv,
    r: u16,
    u: &mut HashSet<String>,
) -> Result<Vec<String>, TeError> {
    let env = PmtcEnvUse::new(env);
    exec_eval_i(t, env, r, u)
}

/// 类似 `exec_eval_use` 但只有 第 2 阶段 (即 不首先 eval 整个 字符串)
pub fn exec_eval_use_2(
    t: &str,
    env: &mut PmtcEnv,
    r: u16,
    u: &mut HashSet<String>,
) -> Result<Vec<String>, TeError> {
    let env = PmtcEnvNoUse::new(env);
    exec_eval_2i(t.to_string(), env, r, u)
}
