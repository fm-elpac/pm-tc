//! 调试/开发 相关命令
use std::process::ExitCode;

use crate::{
    conf::get_eval_rec,
    task,
    te::{self, PmtcEnv},
};

use super::{print_err, print_err_0};

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
    let eval_rec = get_eval_rec();

    for i in a {
        let r = te::eval(&i, &env, eval_rec).map_err(print_err_0)?;
        println!("{}", r);
    }
    Ok(())
}

/// pm-tc --end XXX
pub fn c_end(a: Vec<String>) -> Result<(), ExitCode> {
    for i in a {
        let r = te::end(&i).map_err(print_err_0)?;
        println!("{}", r);
    }
    Ok(())
}

/// pm-tc --exec-eval XXX
pub fn c_exec_eval(a: Vec<String>) -> Result<(), ExitCode> {
    let env = env_debug();
    let eval_rec = get_eval_rec();

    for i in a {
        let r = te::exec_eval(&i, &env, eval_rec).map_err(print_err_0)?;
        // TODO JSON output
        println!("{:#?}", r);
    }
    Ok(())
}

/// pm-tc --env XXX
pub fn c_env(a: Vec<String>) -> Result<(), ExitCode> {
    // TODO
    println!("TODO");
    Ok(())
}
