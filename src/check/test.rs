use super::*;
use std::path::Path;

/// 归一化路径, 转为字符串 (便于断言).
fn n(p: &str) -> String {
    norm_path(Path::new(p)).to_string_lossy().into_owned()
}

// ===== `.` 和 `..` 在 路径中间 的 情况 =====

#[test]
fn norm_cur_dir() {
    assert_eq!(n("./a"), "a");
    assert_eq!(n("a/./b"), "a/b");
    assert_eq!(n("."), "");
}

#[test]
fn norm_parent_middle() {
    assert_eq!(n("a/../b"), "b");
    assert_eq!(n("a/b/../../c"), "c");
    assert_eq!(n("a/../.."), "..");
}

// ===== 以 `..` 开头 (单层 / 多层) 的 情况 =====

#[test]
fn norm_parent_leading() {
    assert_eq!(n("../a"), "../a");
    assert_eq!(n("../../a"), "../../a");
    assert_eq!(n(".."), "..");
    assert_eq!(n("../.."), "../..");
    assert_eq!(n("./.."), "..");
    assert_eq!(n("../."), "..");
}

#[test]
fn norm_parent_absolute() {
    // 绝对路径 不能越过 根目录.
    assert_eq!(n("/../a"), "/a");
    assert_eq!(n("/a/../../b"), "/b");
}
