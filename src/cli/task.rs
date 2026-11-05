//! 执行 task
use std::{env, error::Error, process::ExitCode};

use crate::{
    conf::{ENV_PMTC_CONF, PMTC_DEFAULT_FILE_CONF, ReadConfFile, load},
    f,
    task::{CheckEnv, PlanDebug, PmtcRunList, env_builtin, env_sys, plan, run},
    te::PmtcEnv,
};

use super::print_err;

/// 检查环境 默认实现
pub struct CheckEnvDefault {}

impl CheckEnv for CheckEnvDefault {
    fn file_exist(&mut self, path: &str) -> Result<bool, Box<dyn Error>> {
        f::file_exist(path)
    }

    fn file_read(&mut self, path: &str) -> Result<String, Box<dyn Error>> {
        f::file_read(path)
    }
}

/// 读取配置文件 默认实现
pub struct ReadConfDefault {}

impl ReadConfFile for ReadConfDefault {
    fn read(&mut self, path: &str) -> Result<String, Box<dyn Error>> {
        f::file_read(path)
    }
}

/// 默认 变量 (集)
pub fn env_default() -> PmtcEnv {
    // (1) 内置变量
    let mut o = env_builtin();
    // (2) 加载 `pm_tc.env`保存的变量
    // TODO

    // (3) 系统 环境变量 (覆盖 前面的变量)
    o.add(env_sys());
    o
}

/// 任务执行 (及 调试) 默认实现.
///
/// 调试/实际执行 仅有 后续处理 不同.
pub fn make_plan(a: Vec<String>) -> Result<(PmtcRunList, PlanDebug), Box<dyn Error>> {
    // (环境) 变量
    let envi = env_default();

    // 加载 配置文件
    let file = env::var(ENV_PMTC_CONF).unwrap_or(PMTC_DEFAULT_FILE_CONF.to_string());
    let mut rc = ReadConfDefault {};
    let c = load(&file, &mut rc)?;

    let mut ce = CheckEnvDefault {};

    plan(&a, &c, &envi, &mut ce)
}

/// pm-tc --plan XXX
///
/// (仅) 输出 执行步骤.
pub fn c_plan(a: Vec<String>) -> Result<(), ExitCode> {
    let (r, _) = make_plan(a).map_err(print_err)?;
    // TODO 优化 输出
    println!("{:#?}", r);

    Ok(())
}

/// pm-tc --plan-debug XXX
///
/// 详细 调试 (包含 内部数据).
pub fn c_plan_debug(a: Vec<String>) -> Result<(), ExitCode> {
    let (r, d) = make_plan(a).map_err(print_err)?;
    // TODO 优化 输出
    println!("{:#?}", d);
    println!("{:#?}", r);

    Ok(())
}

/// pm-tc XXX
///
/// 实际执行.
pub fn c_task(a: Vec<String>) -> Result<(), ExitCode> {
    let (r, _) = make_plan(a).map_err(print_err)?;
    // 实际执行
    run(r).map_err(print_err)?;

    Ok(())
}
