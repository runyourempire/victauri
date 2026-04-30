use rmcp::model::{CallToolResult, Content};

/// Produce a properly escaped JavaScript string literal (with double quotes).
/// Uses `serde_json` which handles all special characters: \n, \r, \0, \t,
/// unicode escapes, quotes, backslashes, etc.
pub(crate) fn js_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}

pub(crate) fn json_result(value: &impl serde::Serialize) -> CallToolResult {
    match serde_json::to_string_pretty(value) {
        Ok(json) => CallToolResult::success(vec![Content::text(json)]),
        Err(e) => tool_error(e.to_string()),
    }
}

pub(crate) fn tool_error(msg: impl Into<String>) -> CallToolResult {
    let mut result = CallToolResult::success(vec![Content::text(msg)]);
    result.is_error = Some(true);
    result
}

pub(crate) fn tool_disabled(name: &str) -> CallToolResult {
    tool_error_with_hint(
        format!("tool '{name}' is disabled by privacy configuration"),
        RecoveryHint::ReportToUser,
    )
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum RecoveryHint {
    CheckInput,
    ReportToUser,
}

impl RecoveryHint {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CheckInput => "CHECK_INPUT",
            Self::ReportToUser => "REPORT_TO_USER",
        }
    }
}

pub(crate) fn tool_error_with_hint(msg: impl Into<String>, hint: RecoveryHint) -> CallToolResult {
    let message = msg.into();
    let text = format!(
        "{message}

[hint: {}]",
        hint.as_str()
    );
    let mut result = CallToolResult::success(vec![Content::text(text)]);
    result.is_error = Some(true);
    result
}

pub(crate) fn missing_param(param: &str, action: &str) -> CallToolResult {
    tool_error_with_hint(
        format!("missing required parameter '{param}' for action '{action}'"),
        RecoveryHint::CheckInput,
    )
}

pub(crate) fn validate_url(url: &str) -> Result<(), String> {
    let trimmed: String = url.chars().filter(|c| !c.is_control()).collect();
    match url::Url::parse(&trimmed) {
        Ok(parsed) => match parsed.scheme() {
            "http" | "https" | "file" => Ok(()),
            scheme => Err(format!(
                "scheme '{scheme}' is not allowed; use http, https, or file"
            )),
        },
        Err(e) => Err(format!("invalid URL: {e}")),
    }
}

pub(crate) fn sanitize_css_color(color: &str) -> Result<String, String> {
    let s = color.trim();
    if s.len() > 100 {
        return Err("CSS color value too long".to_string());
    }
    // Reject CSS escape sequences (\XX hex escapes)
    if s.contains('\\') {
        return Err("CSS escape sequences not allowed in color values".to_string());
    }
    let valid = s
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '#' | '(' | ')' | ',' | '.' | ' ' | '%' | '-'));
    if !valid {
        return Err("invalid characters in CSS color value".to_string());
    }
    let lower = s.to_lowercase();
    if lower.contains("url(") || lower.contains("expression(") {
        return Err("invalid CSS color value".to_string());
    }
    Ok(s.to_string())
}
