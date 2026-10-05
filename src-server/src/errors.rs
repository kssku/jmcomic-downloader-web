use serde::Serialize;

use crate::extensions::AnyhowErrorToStringChain;

pub type CommandResult<T> = Result<T, CommandError>;

/// 错误码：整本未下全（缺章）。
///
/// 前端据此把「缺章」渲染成警告色（业务约束），而不是错误色（系统故障）。
/// 用常量而非散落的字面量，避免前后端两边各写一遍字符串。
pub const CODE_INCOMPLETE_CHAPTERS: &str = "INCOMPLETE_CHAPTERS";

#[derive(Debug, Serialize)]
pub struct CommandError {
    pub err_title: String,
    pub err_message: String,
    /// 可选的机器可读错误码。
    ///
    /// `skip_serializing_if` 保证 `None` 时 JSON 里**不出现**该字段——
    /// 历史上所有只设 title/message 的调用点，响应格式逐字不变。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

impl CommandError {
    pub fn from<E>(err_title: &str, err: E) -> Self
    where
        E: Into<anyhow::Error>,
    {
        let string_chain = err.into().to_string_chain();
        tracing::error!(err_title, message = string_chain);
        Self {
            err_title: err_title.to_string(),
            err_message: string_chain,
            code: None,
        }
    }

    /// 与 [`CommandError::from`] 相同，但额外附带一个机器可读错误码。
    ///
    /// 仅在需要前端区分「业务约束」与「系统故障」时使用；其余场景
    /// 继续用 `from`，保持响应体与历史一致。
    pub fn with_code<E>(err_title: &str, err: E, code: &str) -> Self
    where
        E: Into<anyhow::Error>,
    {
        let string_chain = err.into().to_string_chain();
        tracing::error!(err_title, code, message = string_chain);
        Self {
            err_title: err_title.to_string(),
            err_message: string_chain,
            code: Some(code.to_string()),
        }
    }
}
