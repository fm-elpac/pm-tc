//! 底层基础操作 (默认执行器)
//!
//! 处理原则: 当前平台不支持的操作, 不直接报错, 而是替换为 空操作 (nop),
//! 并通过 log::warn!() 输出简短警告 (包含必要参数, 便于调试).
//! 这样 pm-tc 在更多平台上 也能编译通过.
//!
//! 路径约定: pm-tc 内部 (配置文件/库 API) 统一使用 UNIX 形式 路径 (`/` 分隔),
//! 即使 运行在 Windows 上; 本模块 是 唯一 允许 在 Windows 上 接受 非 UNIX 路径
//! (`\`) 的 位置. 这里 的 路径 直接 转交给 系统 文件/命令 API (这些 API 在
//! Windows 上 同时 接受 `/` 与 `\`), 无需 额外 转换.
use std::{error::Error, fmt, fs, io::Write, process::Command};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use log::{debug, error, info, trace, warn};

/// f 模块 自定义错误
#[derive(Debug, Clone)]
pub struct FError(pub String);

impl Error for FError {}

impl fmt::Display for FError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 日志
pub fn log(a: &Vec<String>) -> Result<(), Box<dyn Error>> {
    if a.len() > 0 {
        // 类型
        let t = &a[0];
        // 剩余参数
        let r: Vec<String> = (a[1..]).into();

        match t.as_str() {
            "error" => error!("{:?}", r),
            "warn" => warn!("{:?}", r),
            "info" => info!("{:?}", r),
            "debug" => debug!("{:?}", r),
            "trace" => trace!("{:?}", r),
            // 忽略其余情况
            _ => {}
        }
    }
    // 忽略其余情况
    Ok(())
}

/// 检查 文件 (目录) 是否存在
pub fn file_exist(path: &str) -> Result<bool, Box<dyn Error>> {
    debug!("file_exist {}", path);

    Ok(fs::metadata(path).is_ok())
}

/// 读取文件内容 (utf-8 编码)
pub fn file_read(path: &str) -> Result<String, Box<dyn Error>> {
    debug!("file_read {}", path);

    Ok(fs::read_to_string(path)?)
}

/// 执行外部命令
/// 退出码 0 成功, 否则失败
pub fn exec(a: &Vec<String>) -> Result<(), Box<dyn Error>> {
    info!("exec {:?}", a);

    let (cmd, args) = a
        .split_first()
        .ok_or_else(|| FError("exec: empty arg".to_string()))?;

    let status = Command::new(cmd).args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(FError(format!("exec: exit code {}", status)).into())
    }
}

/// 调用 shell 执行命令
/// 退出码 0 成功, 否则失败
pub fn exec_sh(a: &str) -> Result<(), Box<dyn Error>> {
    info!("exec_sh {}", a);

    let status = Command::new("sh").arg("-c").arg(a).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(FError(format!("exec_sh: exit code {}", status)).into())
    }
}

/// mkdir -p
pub fn mkdir(path: &str) -> Result<(), Box<dyn Error>> {
    info!("mkdir {}", path);

    Ok(fs::create_dir_all(path)?)
}

/// chmod
#[cfg(unix)]
pub fn chmod(path: &str, mode: u32) -> Result<(), Box<dyn Error>> {
    info!("chmod {} {}", mode, path);

    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(mode);
    Ok(fs::set_permissions(path, perms)?)
}

/// chmod (不支持 chmod 的平台)
/// 空操作: 仅输出警告, 便于调试.
#[cfg(not(unix))]
pub fn chmod(path: &str, mode: u32) -> Result<(), Box<dyn Error>> {
    log::warn!("chmod unsupported: mode {:#o} path {}", mode, path);
    Ok(())
}

/// cat
/// 读取 path 对应文件 (原始字节), 写到 stdout
pub fn cat(path: &str) -> Result<(), Box<dyn Error>> {
    debug!("cat {}", path);

    let data = fs::read(path)?;
    let mut out = std::io::stdout();
    out.write_all(&data)?;
    out.flush()?;
    Ok(())
}

/// echo
pub fn echo(text: &str) -> Result<(), Box<dyn Error>> {
    debug!("echo {}", text);

    println!("{}", text);
    Ok(())
}

/// 替换文件内容 (原子 write-mv) (utf-8 编码)
/// 写入中的 临时文件 为 `[path].pm_tc_tmp`
pub fn file_replace(path: &str, text: &str) -> Result<(), Box<dyn Error>> {
    info!("file_replace {}", path);
    debug!("file_replace text {}", text);

    let tmp = format!("{}.pm_tc_tmp", path);
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// 追加写入文件 (utf-8 编码)
pub fn file_append(path: &str, text: &str) -> Result<(), Box<dyn Error>> {
    info!("file_append {}", path);
    debug!("file_append text {}", text);

    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    f.write_all(text.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod test;
