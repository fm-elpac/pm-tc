use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU32, Ordering},
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use super::*;

static N: AtomicU32 = AtomicU32::new(0);

/// 在 `/tmp` 下创建 唯一 的 临时目录, 测试结束后 (包括 panic) 自动删除.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let d = std::env::temp_dir().join(format!(
            "pm_tc_f_test_{}_{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&d).unwrap();
        TempDir(d)
    }

    fn join(&self, p: &str) -> PathBuf {
        self.0.join(p)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn mkdir_and_exist() {
    let t = TempDir::new();
    let sub = t.join("x/y/z");
    let sub = sub.to_str().unwrap();

    assert!(!file_exist(sub).unwrap());
    mkdir(sub).unwrap();
    assert!(file_exist(sub).unwrap());
}

#[test]
fn file_replace_append_read() {
    let t = TempDir::new();
    let p = t.join("a.txt");
    let p = p.to_str().unwrap();

    file_replace(p, "hello").unwrap();
    assert_eq!(file_read(p).unwrap(), "hello");

    file_append(p, " world").unwrap();
    assert_eq!(file_read(p).unwrap(), "hello world");
}

#[cfg(unix)]
#[test]
fn chmod_works() {
    let t = TempDir::new();
    let p = t.join("f");
    fs::write(&p, "x").unwrap();

    chmod(p.to_str().unwrap(), 0o700).unwrap();
    let mode = fs::metadata(&p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700);
}

#[cfg(unix)]
#[test]
fn exec_and_exec_sh() {
    exec(&vec!["true".to_string()]).unwrap();
    exec_sh("exit 0").unwrap();
}

#[test]
fn log_noop() {
    // 空参数 / 未知级别 应被忽略, 不报错.
    log(&vec![]).unwrap();
    log(&vec!["other".to_string()]).unwrap();
    log(&vec!["debug".to_string(), "a".to_string()]).unwrap();
}

#[cfg(unix)]
#[test]
fn exec_err() {
    assert!(exec(&vec!["false".to_string()]).is_err());
}

#[test]
fn echo_and_cat() {
    echo("hi").unwrap();

    let t = TempDir::new();
    let p = t.join("c.txt");
    fs::write(&p, "cat-data").unwrap();
    cat(p.to_str().unwrap()).unwrap();
}
