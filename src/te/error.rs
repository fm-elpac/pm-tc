//! 错误处理
use std::{error::Error, fmt};

/// te 错误.
///
/// 其中 `pos: usize` 是 错误位置 (char).
#[derive(Debug, Clone)]
pub enum TeError {
    /// 错误的 转义序列.
    /// 只允许 3 种 转义序列: `\\` (转义 转义字符 自身), `\[` (`[`), `\]` (`]`).
    ///
    /// (pos, char) pos 指向 `\` 字符. char 为 `\` 后面的字符.
    /// 如果 `\` 位于字符串结束 (没有后续字符), 则 char 为 None.
    Escape((usize, Option<char>)),
    /// 变量名称 错误: 不允许的字符.
    ///
    /// (pos, char) pos 指向 不允许的字符. char 为 不允许的字符.
    NameChar((usize, char)),
    /// 变量名称 错误: NFKC 之后发生变化.
    ///
    /// (pos, before, after) pos 指向 变量名 开始字符.
    NameNFKC((usize, String, String)),
    /// 变量名称 错误: 不允许为空.
    ///
    /// (pos)
    NameEmpty(usize),
    /// 模板 格式错误: 没有匹配的 `]`.
    ///
    /// (pos, t) pos 指向 `[`.
    /// `t` 是从 `[` 开始的文本 (不含 `[`).
    VE((usize, String)),
    /// 模板 (变量) 格式错误: 不符合语法.
    /// 目前只支持这些语法: `[NAME]`, `[:NAME]`, `[n:NAME]` (n >= 2).
    ///
    ///  (pos, t) pos 指向 `[`.
    /// 其中 `t` 是 `[` 和 `]` 之间的文本 (不含 `[` 和 `]`).
    VF((usize, String)),
    /// `eval()` 递归深度超出: 耗尽递归次数.
    ///
    /// (r, s) r 是允许的最大递归次数, s 是 递归调用栈 (捕获每次的 模板)
    EvalRec((u16, Vec<String>)),
}

impl Error for TeError {}

/// 此处的显示方式只是 缺省/默认 显示, 适用于 开发/调试.
/// 调用者 可以使用对用户更友好的显示方式.
impl fmt::Display for TeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Escape((pos, c)) => write!(f, "TeError::Escape ({}) `{:?}`", pos, c),
            Self::NameChar((pos, c)) => write!(f, "TeError::NameChar ({}) `{}`", pos, c),
            Self::NameNFKC((pos, before, after)) => {
                write!(f, "TeError::NameNFKC ({}) `{}` -> `{}`", pos, before, after)
            }
            Self::NameEmpty(pos) => write!(f, "TeError::NameEmpty ({})", pos),
            Self::VE((pos, t)) => write!(f, "TeError::VE ({}) `{}`", pos, t),
            Self::VF((pos, t)) => write!(f, "TeError::VF ({}) `{}`", pos, t),
            Self::EvalRec((r, s)) => write!(f, "TeError::EvalRec ({}) {:?}", r, s),
        }
    }
}
