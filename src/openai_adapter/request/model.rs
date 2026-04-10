//! 模型解析 —— 将 OpenAI model 字段映射为 ds_core 能力标志
//!
//! 当前仅支持 deepseek-default 与 deepseek-expert 两个模型别名。

use crate::openai_adapter::types::WebSearchOptions;

/// 模型解析结果
pub struct ModelResolution {
    // TODO: 待 ds_core 支持 model_type 输入参数后启用
    // pub model_type: String,
    pub thinking_enabled: bool,
    pub search_enabled: bool,
}

/// 根据 model_id 和扩展参数解析模型配置
///
/// thinking_enabled 在 reasoning_effort 为 minimal/low/medium/high/xhigh 时启用。
/// search_enabled 在 web_search_options 存在时启用。
pub fn resolve(
    model_id: &str,
    reasoning_effort: Option<&str>,
    web_search_options: Option<&WebSearchOptions>,
) -> Result<ModelResolution, String> {
    let _model_type = if model_id.eq_ignore_ascii_case("deepseek-default") {
        "default"
    } else if model_id.eq_ignore_ascii_case("deepseek-expert") {
        "expert"
    } else {
        return Err(format!("不支持的模型: {}", model_id));
    };

    let thinking_enabled = matches!(
        reasoning_effort,
        Some("minimal" | "low" | "medium" | "high" | "xhigh")
    );

    let search_enabled = web_search_options.is_some();

    Ok(ModelResolution {
        // model_type: _model_type.into(),
        thinking_enabled,
        search_enabled,
    })
}
