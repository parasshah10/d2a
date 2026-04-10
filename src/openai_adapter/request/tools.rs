//! 工具解析 —— 校验 tools/tool_choice 并生成提示词注入文本
//!
//! 由于 ds_core 不支持原生 function calling，本模块将工具定义降级为
//! 自然语言描述，并追加到 prompt 中引导模型输出。

use crate::openai_adapter::types::{
    AllowedTools, AllowedToolsChoice, ChatCompletionRequest, CustomTool, CustomToolFormat,
    FunctionDefinition, Tool, ToolChoice,
};

/// 提取后的工具上下文
pub struct ToolContext {
    /// 格式化后的工具定义文本
    pub defs_text: Option<String>,
    /// 根据 tool_choice / parallel_tool_calls 追加的行为指令
    pub instruction_text: Option<String>,
}

fn has_tools(req: &ChatCompletionRequest) -> bool {
    req.tools.as_ref().map(|t| !t.is_empty()).unwrap_or(false)
}

/// 从请求中提取并校验工具信息
///
/// 当 tool_choice 为 none 时返回空的 ToolContext，不生成任何注入文本。
pub fn extract(req: &ChatCompletionRequest) -> Result<ToolContext, String> {
    let default_choice = if has_tools(req) {
        ToolChoice::Mode("auto".to_string())
    } else {
        ToolChoice::Mode("none".to_string())
    };
    let tool_choice = req.tool_choice.as_ref().unwrap_or(&default_choice);

    validate_tool_choice(tool_choice, req.tools.as_deref())?;

    if matches!(tool_choice, ToolChoice::Mode(m) if m == "none") {
        return Ok(ToolContext {
            defs_text: None,
            instruction_text: None,
        });
    }

    let mut instruction_lines = Vec::new();

    match tool_choice {
        ToolChoice::Mode(mode) => {
            if mode == "required" {
                instruction_lines.push("注意：你必须调用一个或多个工具。".to_string());
            }
        }
        ToolChoice::AllowedTools(AllowedToolsChoice { allowed_tools, .. }) => {
            build_allowed_tools_instruction(allowed_tools, &mut instruction_lines)?;
        }
        ToolChoice::Named(named) => {
            instruction_lines.push(format!("注意：你必须调用 '{}' 工具。", named.function.name));
        }
        ToolChoice::Custom(custom) => {
            instruction_lines.push(format!(
                "注意：你必须调用 '{}' 自定义工具。",
                custom.custom.name
            ));
        }
    }

    if req.parallel_tool_calls == Some(false) {
        instruction_lines.push("注意：一次只能调用一个工具。".to_string());
    }

    instruction_lines.push(
        "注意：当需要调用工具时，请严格按如下 XML 格式输出，不要添加任何解释性文字：".to_string(),
    );
    instruction_lines.push("<tool_calls>".to_string());
    instruction_lines.push("<tool_call name=\"工具名\" arguments=\"{JSON参数}\" />".to_string());
    instruction_lines.push("</tool_calls>".to_string());
    instruction_lines
        .push("多个工具调用可在同一 <tool_calls> 标签内列出多个 <tool_call />。".to_string());

    let defs_text = if has_tools(req) {
        let mut lines = vec!["你可以使用以下工具：".to_string()];
        for (i, tool) in req.tools.as_ref().unwrap().iter().enumerate() {
            lines.push(format_tool(tool, i)?);
        }
        Some(lines.join("\n"))
    } else {
        None
    };

    let instruction_text = if instruction_lines.is_empty() {
        None
    } else {
        Some(instruction_lines.join("\n"))
    };

    Ok(ToolContext {
        defs_text,
        instruction_text,
    })
}

fn validate_tool_choice(tc: &ToolChoice, tools: Option<&[Tool]>) -> Result<(), String> {
    match tc {
        ToolChoice::Mode(mode) => {
            if !matches!(mode.as_str(), "none" | "auto" | "required") {
                return Err(format!("tool_choice 无效模式: {}", mode));
            }
            if matches!(mode.as_str(), "auto" | "required")
                && tools.map(|t| t.is_empty()).unwrap_or(true)
            {
                return Err("tool_choice 为 'auto' 或 'required' 时必须提供 tools".into());
            }
            Ok(())
        }
        ToolChoice::Named(_) | ToolChoice::Custom(_) => {
            if tools.is_none() {
                return Err("tool_choice 指定了具体工具时必须提供 tools".into());
            }
            Ok(())
        }
        ToolChoice::AllowedTools(AllowedToolsChoice { allowed_tools, .. }) => {
            if tools.is_none() {
                return Err("tool_choice 指定了 allowed_tools 时必须提供 tools".into());
            }
            if !matches!(allowed_tools.mode.as_str(), "auto" | "required") {
                return Err(format!(
                    "allowed_tools.mode 必须是 'auto' 或 'required'，收到: {}",
                    allowed_tools.mode
                ));
            }
            Ok(())
        }
    }
}

fn build_allowed_tools_instruction(
    allowed_tools: &AllowedTools,
    lines: &mut Vec<String>,
) -> Result<(), String> {
    if let Some(tool_list) = &allowed_tools.tools {
        let names: Vec<String> = tool_list
            .iter()
            .filter_map(|v| v.get("function").and_then(|f| f.get("name")))
            .filter_map(|n| n.as_str().map(|s| s.to_string()))
            .collect();
        if !names.is_empty() {
            lines.push(format!(
                "注意：你只能从以下允许的工具中选择：{}。",
                names.join(", ")
            ));
        }
    }

    if allowed_tools.mode == "required" {
        lines.push("注意：你必须调用一个或多个工具。".to_string());
    }
    Ok(())
}

fn format_tool(tool: &Tool, idx: usize) -> Result<String, String> {
    match tool.ty.as_str() {
        "function" => {
            let func = tool.function.as_ref().ok_or_else(|| {
                format!("tools[{}] 类型为 'function' 时必须提供 function 定义", idx)
            })?;
            format_function(func)
        }
        "custom" => {
            let custom = tool
                .custom
                .as_ref()
                .ok_or_else(|| format!("tools[{}] 类型为 'custom' 时必须提供 custom 定义", idx))?;
            Ok(format_custom(custom))
        }
        _ => Err(format!("tools[{}] 不支持的类型: {}", idx, tool.ty)),
    }
}

fn format_function(func: &FunctionDefinition) -> Result<String, String> {
    if func.name.trim().is_empty() {
        return Err("tools 中 function 缺少必填字段 'name'".into());
    }
    let params = serde_json::to_string(&func.parameters).unwrap_or_else(|_| "{}".into());
    Ok(format!(
        "- {} (function): {}\n  参数(JSON schema): {}",
        func.name,
        func.description.as_deref().unwrap_or(""),
        params
    ))
}

fn format_custom(custom: &CustomTool) -> String {
    let format_desc: &str = match &custom.format {
        Some(CustomToolFormat::Text) => "text",
        Some(CustomToolFormat::Grammar { grammar }) => {
            return format!(
                "- {} (custom): {}\n  格式: grammar(syntax: {})",
                custom.name,
                custom.description.as_deref().unwrap_or(""),
                grammar.syntax
            );
        }
        None => "无约束",
    };
    format!(
        "- {} (custom): {}\n  格式: {}",
        custom.name,
        custom.description.as_deref().unwrap_or(""),
        format_desc
    )
}
