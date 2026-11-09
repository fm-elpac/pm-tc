use super::*;

/// 归一化路径, 转为字符串 (便于断言).
fn n(p: &str) -> String {
    norm_path(p)
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

#[test]
fn to_unix_path_works() {
    assert_eq!(to_unix_path("a/b".into()), "a/b");
    // 在 Windows 上 `\` 会被转换为 `/`; 在 UNIX 上 本函数 是 空操作.
    #[cfg(windows)]
    assert_eq!(to_unix_path("a\\b".into()), "a/b");
}
