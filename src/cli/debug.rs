//! 调试/开发 相关命令
use std::{
    collections::{HashMap, HashSet},
    process::ExitCode,
};

use crate::{
    conf::{PMTC_DEFAULT_EVAL_REC, SrcInfo},
    task,
    te::{self, PMTC_ENV_LABEL_SYS, PmtcEnv},
};

use super::{eval_rec, json_out, print_err, print_err_0};

/// 用于调试的 默认 变量 (集)
pub fn env_debug() -> PmtcEnv {
    // (1) 内置变量
    let mut o = task::env_builtin();
    // (2) 系统 环境变量 (覆盖 内置变量)
    o.add(task::env_sys());

    o
}

/// pm-tc --eval XXX
pub fn c_eval(a: Vec<String>) -> Result<(), ExitCode> {
    let env = env_debug();
    let eval_r = eval_rec(PMTC_DEFAULT_EVAL_REC);
    let j = json_out();
    let src = SrcInfo::new();

    for i in a {
        let r = te::eval(&i, &env, eval_r).map_err(|e| print_err_0(e, &src))?;
        if j {
            let o = serde_json::to_string(&r).map_err(|e| print_err_0(e, &src))?;
            println!("{}", o);
        } else {
            println!("{}", r);
        }
    }
    Ok(())
}

/// pm-tc --end XXX
pub fn c_end(a: Vec<String>) -> Result<(), ExitCode> {
    let j = json_out();
    let src = SrcInfo::new();

    for i in a {
        let r = te::end(&i).map_err(|e| print_err_0(e, &src))?;
        if j {
            let o = serde_json::to_string(&r).map_err(|e| print_err_0(e, &src))?;
            println!("{}", o);
        } else {
            println!("{}", r);
        }
    }
    Ok(())
}

/// pm-tc --exec-eval XXX
pub fn c_exec_eval(a: Vec<String>) -> Result<(), ExitCode> {
    let env = env_debug();
    let eval_r = eval_rec(PMTC_DEFAULT_EVAL_REC);
    let j = json_out();
    let src = SrcInfo::new();

    for i in a {
        let r = te::exec_eval(&i, &env, eval_r).map_err(|e| print_err_0(e, &src))?;
        if j {
            let o = serde_json::to_string(&r).map_err(|e| print_err_0(e, &src))?;
            println!("{}", o);
        } else {
            println!("{:#?}", r);
        }
    }
    Ok(())
}

/// pm-tc --env [任务 ...]
///
/// 使用 参数 (可为空) 执行 `task::plan`, 输出 所有 被使用的 系统环境变量
/// (`HashMap<String, String>`), 结果使用 pretty JSON (2 空格 缩进).
pub fn c_env(a: Vec<String>) -> Result<(), ExitCode> {
    // 与 普通任务执行 使用 相同的 配置/环境 加载逻辑.
    let mut src = SrcInfo::new();
    let (_, pd) = super::task::make_plan(a, &mut src).map_err(|e| print_err(e, &src))?;

    // 收集 被使用的 系统环境变量 名称.
    let mut names: HashSet<String> = HashSet::new();
    for (name, label) in pd.env0.use_get() {
        if PMTC_ENV_LABEL_SYS == label {
            names.insert(name);
        }
    }
    for t in &pd.task {
        for (name, label) in &t.env_use {
            if PMTC_ENV_LABEL_SYS == *label {
                names.insert(name.clone());
            }
        }
    }

    // 读取 系统环境变量 的 值.
    let sys = task::env_sys();
    let mut out: HashMap<String, String> = HashMap::new();
    for name in names {
        if let Some(value) = sys.get(&name) {
            out.insert(name, value);
        }
    }

    let j = serde_json::to_string_pretty(&out).map_err(|e| print_err_0(e, &src))?;
    println!("{}", j);
    Ok(())
}
