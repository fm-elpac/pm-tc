//! 英文 友好错误信息.
use std::error::Error;

use crate::{ConfError, TaskError, TeError, conf::SrcInfo};

use super::{OwnErr, classify, pos, render};

/// 英文 友好错误显示.
///
/// 见 [`super::t_err`].
pub fn t_err(err: &Box<dyn Error>, src: &SrcInfo) -> Option<String> {
    match classify(err.as_ref())? {
        OwnErr::Conf(c) => conf_msg(c, src),
        OwnErr::Task(t) => task_msg(t, src),
        OwnErr::Te(e) => Some(render(None, te_msg(e))),
    }
}

fn conf_msg(e: &ConfError, src: &SrcInfo) -> Option<String> {
    match e {
        ConfError::FileId(id) => Some(render(
            None,
            format!("internal error: invalid config file id {}", id),
        )),
        ConfError::Toml(p) => Some(render(pos(src, &p.src()), p.as_ref().message().to_string())),
        ConfError::Te(p) => Some(render(pos(src, &p.src()), te_msg(p.as_ref()))),
        ConfError::ConfVersion((p, _)) => Some(render(pos(src, &p.src()), p.as_ref().clone())),
        ConfError::Key((p, _)) => Some(render(pos(src, &p.src()), p.as_ref().clone())),
        ConfError::Value((p, _)) => Some(render(pos(src, &p.src()), p.as_ref().clone())),
        ConfError::IncludeDup((p, _)) => Some(render(pos(src, &p.src()), p.as_ref().clone())),
        ConfError::IncludeCycle((p, _)) => Some(render(pos(src, &p.src()), p.as_ref().clone())),
        ConfError::DepCycle((p, _)) => Some(render(pos(src, &p.src()), p.as_ref().clone())),
        ConfError::FA((p, _)) => Some(render(pos(src, &p.src()), p.as_ref().clone())),
        ConfError::FAEmpty((p, _)) => Some(render(pos(src, &p.src()), p.as_ref().clone())),
        ConfError::DepT((p, _)) => Some(render(pos(src, &p.src()), p.as_ref().clone())),
        ConfError::DepF((p, _)) => Some(render(pos(src, &p.src()), p.as_ref().clone())),
        ConfError::F((p, _)) => Some(render(pos(src, &p.src()), p.as_ref().clone())),
        ConfError::FDup((p, _)) => Some(render(pos(src, &p.src()), p.as_ref().clone())),
    }
}

fn task_msg(e: &TaskError, src: &SrcInfo) -> Option<String> {
    match e {
        TaskError::DepT((p, _)) => Some(render(pos(src, &p.src()), p.as_ref().clone())),
        TaskError::F((p, _)) => Some(render(pos(src, &p.src()), p.as_ref().clone())),
        TaskError::FDef((p, _)) => Some(render(pos(src, &p.src()), p.as_ref().clone())),
        TaskError::Arg((p, _)) => Some(render(pos(src, &p.src()), p.as_ref().clone())),
        TaskError::DepCycle((p, _)) => Some(render(pos(src, &p.src()), p.as_ref().clone())),
        TaskError::FileWrite((p, _)) => Some(render(pos(src, &p.src()), p.as_ref().clone())),
        TaskError::Te(p) => Some(render(pos(src, &p.src()), te_msg(p.as_ref()))),
    }
}

fn te_msg(e: &TeError) -> String {
    match e {
        TeError::Escape((pos, c)) => match c {
            Some(c) => format!("invalid escape sequence at {}: `{}`", pos, c),
            None => format!("invalid escape sequence at {}", pos),
        },
        TeError::NameChar((pos, c)) => {
            format!("invalid variable name character at {}: `{}`", pos, c)
        }
        TeError::NameNFKC((pos, before, after)) => {
            format!(
                "variable name changes after NFKC at {}: `{}` -> `{}`",
                pos, before, after
            )
        }
        TeError::NameEmpty(pos) => format!("empty variable name at {}", pos),
        TeError::VE((pos, t)) => format!("missing closing `]` at {}: `{}`", pos, t),
        TeError::VF((pos, t)) => format!("invalid template syntax at {}: `{}`", pos, t),
        TeError::EvalRec((r, _)) => format!("template recursion limit {} exceeded", r),
    }
}
