use rmcp::model::{CallToolResult, Content};
use crate::server::errors::AppError;

pub fn app_error_result(err: AppError) -> Result<CallToolResult, rmcp::ErrorData> {
    Ok(CallToolResult::error(vec![Content::text(err.to_string())]))
}

pub fn str_error_result(msg: impl std::fmt::Display) -> Result<CallToolResult, rmcp::ErrorData> {
    Ok(CallToolResult::error(vec![Content::text(msg.to_string())]))
}
