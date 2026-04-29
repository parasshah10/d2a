//! Prompt 构建 —— 将 OpenAI messages 转换为 DeepSeek 原生标签格式
//!
//! 使用 `<｜System｜>`、`<｜User｜>`、`<｜Assistant｜>`、`<｜Tool｜>` 作为角色标记。
//! 若请求包含工具定义或行为指令，会嵌入到最后一个 `<｜Assistant｜>` 后的
//! 不闭合 `<think>` 块中，确保工具上下文始终紧邻模型生成位置。

use super::tools::ToolContext;
use crate::openai_adapter::response::{TOOL_CALL_END, TOOL_CALL_START};
use crate::openai_adapter::types::{ChatCompletionsRequest, ContentPart, Message, MessageContent};

/// 构建 DeepSeek 原生标签格式的 prompt 字符串
/// 顺序: [system] [历史 user/tool/assistant 轮次...] <｜Assistant｜><think>[reminder]
pub fn build(req: &ChatCompletionsRequest, tool_ctx: &ToolContext) -> String {
    let mut parts: Vec<String> = req.messages.iter().map(format_message).collect();

    let mut tool_sections: Vec<String> = Vec::new();

    if let Some(text) = tool_ctx.format_block.as_deref() {
        tool_sections.push(format!("### 格式规范\n{}", text));
    }
    if let Some(text) = tool_ctx.defs_text.as_deref() {
        tool_sections.push(format!("### 工具定义\n{}", text));
    }
    if let Some(text) = tool_ctx.instruction_text.as_deref() {
        tool_sections.push(format!("### 调用指令\n{}", text));
    }

    let mut reminder_parts: Vec<String> = Vec::new();

    if !tool_sections.is_empty() {
        reminder_parts.push(format!("## 工具调用\n{}", tool_sections.join("\n\n")));
    }

    // response_format 降级：将格式约束注入到 <think> 块中
    if let Some(rf) = &req.response_format {
        let text = match rf.ty.as_str() {
            "json_object" => {
                "请直接输出合法的 JSON 对象，不要包含任何 markdown 代码块标记或其他解释性文字。"
                    .into()
            }
            "json_schema" => {
                let schema_text = rf
                    .json_schema
                    .as_ref()
                    .map(|s| serde_json::to_string(s).unwrap_or_default())
                    .unwrap_or_default();
                if schema_text.is_empty() {
                    "以 JSON 的形式输出。".into()
                } else {
                    format!(
                        "以 JSON 的形式输出，输出的 JSON 需遵守以下的格式：\n\n~~~json\n{}\n~~~",
                        schema_text
                    )
                }
            }
            "text" => String::new(),
            _ => format!("请以 {} 格式输出。", rf.ty),
        };
        if !text.is_empty() {
            reminder_parts.push(format!("## 输出格式\n{}", text));
        }
    }

    // 工具调用包裹指令
    if tool_ctx.defs_text.is_some() {
        reminder_parts.push(format!(
            "(工具调用请使用 {TOOL_CALL_START} 和 {TOOL_CALL_END} 包裹。)"
        ));
    }

    if !reminder_parts.is_empty() {
        let reminder = format!("我被系统提醒如下信息:\n\n{}", reminder_parts.join("\n\n"));
        parts.push(format!("<｜Assistant｜><think>{}\n", reminder));
    }

    parts.join("")
}

fn role_tag(role: &str) -> String {
    let mut r = role.to_string();
    if let Some(c) = r.get_mut(0..1) {
        c.make_ascii_uppercase();
    }
    format!("<｜{}｜>", r)
}

fn format_message(msg: &Message) -> String {
    let body = match msg.role.as_str() {
        "assistant" => format_assistant(msg),
        "tool" => format_tool(msg),
        "function" => format_function(msg),
        _ => format_generic(msg),
    };
    let tag = role_tag(&msg.role);
    format!("{}{}\n", tag, body)
}

fn format_generic(msg: &Message) -> String {
    let mut parts = Vec::new();
    if let Some(name) = &msg.name {
        parts.push(format!("(name: {name})"));
    }
    if let Some(content) = &msg.content {
        parts.push(format_content(content));
    }
    parts.join("\n")
}

fn format_assistant(msg: &Message) -> String {
    let mut parts = Vec::new();
    if let Some(content) = &msg.content {
        parts.push(format_content(content));
    }
    if let Some(tool_calls) = &msg.tool_calls {
        let items: Vec<String> = tool_calls
            .iter()
            .filter_map(|tc| {
                tc.function.as_ref().map(|func| {
                    let args = serde_json::from_str::<serde_json::Value>(&func.arguments)
                        .unwrap_or(serde_json::Value::Null);
                    format!(
                        "{{\"name\": {}, \"arguments\": {}}}",
                        serde_json::to_string(&func.name).unwrap_or_else(|_| "\"\"".into()),
                        serde_json::to_string(&args).unwrap_or_else(|_| "null".into()),
                    )
                })
            })
            .collect();
        parts.push(format!(
            "{TOOL_CALL_START}\n[{}]\n{TOOL_CALL_END}",
            items.join(", ")
        ));
    }
    if let Some(fc) = &msg.function_call {
        let args = serde_json::from_str::<serde_json::Value>(&fc.arguments)
            .unwrap_or(serde_json::Value::Null);
        let item = format!(
            "{{\"name\": {}, \"arguments\": {}}}",
            serde_json::to_string(&fc.name).unwrap_or_else(|_| "\"\"".into()),
            serde_json::to_string(&args).unwrap_or_else(|_| "null".into()),
        );
        parts.push(format!("{TOOL_CALL_START}\n[{item}]\n{TOOL_CALL_END}"));
    }
    if let Some(refusal) = &msg.refusal {
        parts.push(format!("(refusal: {refusal})"));
    }
    parts.join("\n")
}

fn format_tool(msg: &Message) -> String {
    let mut parts = Vec::new();
    parts.push("# 工具调用结果".to_string());
    if let Some(name) = &msg.name {
        parts.push(format!("## 工具名称: {}", name));
    }
    if let Some(id) = &msg.tool_call_id {
        parts.push(format!("## 调用id: {}", id));
    }
    parts.push("## 调用结果:".to_string());
    if let Some(content) = &msg.content {
        parts.push("~~~".to_string());
        parts.push(format_content(content));
        parts.push("~~~".to_string());
    }
    parts.join("\n")
}

fn format_function(msg: &Message) -> String {
    let mut parts = Vec::new();
    if let Some(name) = &msg.name {
        parts.push(format!("(name: {name})"));
    }
    if let Some(content) = &msg.content {
        parts.push(format_content(content));
    }
    parts.join("\n")
}

fn format_content(content: &MessageContent) -> String {
    match content {
        MessageContent::Text(text) => text.clone(),
        MessageContent::Parts(parts) => {
            parts.iter().map(format_part).collect::<Vec<_>>().join("\n")
        }
    }
}

fn format_part(part: &ContentPart) -> String {
    match part.ty.as_str() {
        "text" => part.text.clone().unwrap_or_default(),
        "refusal" => part.refusal.clone().unwrap_or_default(),
        "image_url" => {
            let detail = part
                .image_url
                .as_ref()
                .and_then(|i| i.detail.as_deref())
                .unwrap_or("auto");
            format!("[图片: detail={detail}]")
        }
        "input_audio" => {
            let fmt = part
                .input_audio
                .as_ref()
                .map(|a| a.format.as_str())
                .unwrap_or("unknown");
            format!("[音频: format={fmt}]")
        }
        "file" => {
            let filename = part
                .file
                .as_ref()
                .and_then(|f| f.filename.as_deref())
                .unwrap_or("unknown");
            format!("[文件: filename={filename}]")
        }
        _ => format!("[未支持的内容类型: {}]", part.ty),
    }
}
