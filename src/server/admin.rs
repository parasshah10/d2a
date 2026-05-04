//! 管理 API 路由处理器 —— 登录/设置密码、账号池状态、请求统计、模型列表、配置查看

use axum::{
    body::Body,
    extract::{Query, State},
    http::{StatusCode, header},
    response::Response,
};
use serde::{Deserialize, Serialize};

use super::handlers::AppState;
use crate::config::Config;

// ── 请求/响应类型 ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SetupRequest {
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
}

#[derive(Serialize)]
pub struct AdminStatusResponse {
    pub accounts: Vec<crate::ds_core::AccountStatus>,
    pub total: usize,
    pub idle: usize,
    pub busy: usize,
    pub error: usize,
    pub invalid: usize,
}

#[derive(Serialize)]
pub struct AdminStatsResponse {
    #[serde(flatten)]
    pub stats: super::stats::StatsSnapshot,
}

#[derive(Serialize)]
pub struct AdminConfigResponse {
    pub server: ServerConfigView,
    pub deepseek: DeepSeekConfigView,
    pub accounts: Vec<AccountView>,
}

#[derive(Serialize)]
pub struct ServerConfigView {
    pub host: String,
    pub port: u16,
}

#[derive(Serialize)]
pub struct DeepSeekConfigView {
    pub api_base: String,
    pub model_types: Vec<String>,
    pub max_input_tokens: Vec<u32>,
    pub max_output_tokens: Vec<u32>,
}

#[derive(Serialize)]
pub struct AccountView {
    pub email: String,
    pub mobile: String,
    pub area_code: String,
    pub password: String,
}

// ── 脱敏 ─────────────────────────────────────────────────────────────────

fn mask_config(config: &Config) -> AdminConfigResponse {
    AdminConfigResponse {
        server: ServerConfigView {
            host: config.server.host.clone(),
            port: config.server.port,
        },
        deepseek: DeepSeekConfigView {
            api_base: config.deepseek.api_base.clone(),
            model_types: config.deepseek.model_types.clone(),
            max_input_tokens: config.deepseek.max_input_tokens.clone(),
            max_output_tokens: config.deepseek.max_output_tokens.clone(),
        },
        accounts: config
            .accounts
            .iter()
            .map(|a| AccountView {
                email: a.email.clone(),
                mobile: a.mobile.clone(),
                area_code: a.area_code.clone(),
                password: "***".to_string(),
            })
            .collect(),
    }
}

// ── Handlers ──────────────────────────────────────────────────────────────

/// POST /admin/api/setup — 首次设置密码
pub(crate) async fn admin_setup(
    State(state): State<AppState>,
    body: axum::body::Bytes,
) -> Response {
    let req: SetupRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("请求格式错误: {}", e)),
    };

    match super::auth::setup_admin(&state.store, &state.login_limiter, &req.password).await {
        Ok(token) => json_response(&LoginResponse { token }),
        Err(msg) => {
            let status = if msg.contains("已设置") {
                StatusCode::FORBIDDEN
            } else if msg.contains("次数过多") {
                StatusCode::TOO_MANY_REQUESTS
            } else if msg.contains("至少 6 位") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            error_response(status, &msg)
        }
    }
}

pub(crate) async fn admin_login(
    State(state): State<AppState>,
    body: axum::body::Bytes,
) -> Response {
    let req: LoginRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("请求格式错误: {}", e)),
    };

    match super::auth::login_admin(&state.store, &state.login_limiter, &req.password).await {
        Ok(token) => json_response(&LoginResponse { token }),
        Err(msg) => {
            let status = if msg.contains("次数过多") {
                StatusCode::TOO_MANY_REQUESTS
            } else if msg.contains("未设置密码") {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::UNAUTHORIZED
            };
            error_response(status, &msg)
        }
    }
}

/// GET /admin/api/status
pub(crate) async fn admin_status(State(state): State<AppState>) -> Response {
    let statuses = state.adapter.account_statuses();
    let total = statuses.len();
    let busy = statuses.iter().filter(|a| a.state == "busy").count();
    let idle = statuses.iter().filter(|a| a.state == "idle").count();
    let error = statuses.iter().filter(|a| a.state == "error").count();
    let invalid = statuses.iter().filter(|a| a.state == "invalid").count();

    let resp = AdminStatusResponse {
        accounts: statuses,
        total,
        idle,
        busy,
        error,
        invalid,
    };
    json_response(&resp)
}

/// GET /admin/api/stats
pub(crate) async fn admin_stats(State(state): State<AppState>) -> Response {
    let snapshot = state.stats.snapshot();
    let resp = AdminStatsResponse { stats: snapshot };
    json_response(&resp)
}

/// GET /admin/api/models
pub(crate) async fn admin_models(State(state): State<AppState>) -> Response {
    let models = state.adapter.list_models().await;
    json_response(&models)
}

/// GET /admin/api/config
pub(crate) async fn admin_config(State(state): State<AppState>) -> Response {
    let config = state.config.read().await;
    let config_view = mask_config(&config);
    json_response(&config_view)
}

/// PUT /admin/api/config — 更新并热重载配置
pub(crate) async fn admin_put_config(
    State(state): State<AppState>,
    body: axum::body::Bytes,
) -> Response {
    let new_config: Config = match serde_json::from_slice(&body) {
        Ok(c) => c,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("JSON 解析失败: {}", e)),
    };

    // Validate
    if let Err(e) = new_config.validate() {
        return error_response(StatusCode::BAD_REQUEST, &e.to_string());
    }

    // Persist
    {
        let mut guard = state.config.write().await;
        *guard = new_config.clone();
        if let Err(e) = guard.save(&state.config_path) {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("保存失败: {}", e),
            );
        }
    }

    // Hot-reload: sync accounts from the new config
    state.adapter.sync_accounts(&new_config.accounts).await;
    json_response(&serde_json::json!({"ok": true}))
}

#[derive(Deserialize)]
pub struct LogsQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    50
}

/// GET /admin/api/logs — 获取最近的请求日志
pub(crate) async fn admin_logs(
    Query(query): Query<LogsQuery>,
    State(state): State<AppState>,
) -> Response {
    let logs = state.stats.recent_logs(query.limit);
    json_response(&logs)
}

#[derive(Deserialize)]
pub struct RuntimeLogsQuery {
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_runtime_limit")]
    pub limit: usize,
}

fn default_runtime_limit() -> usize {
    100
}

/// GET /admin/api/runtime-logs — 分页查询运行日志
pub(crate) async fn admin_runtime_logs(Query(query): Query<RuntimeLogsQuery>) -> Response {
    let (total, logs) = super::runtime_log::query_logs(query.offset, query.limit).await;
    json_response(&serde_json::json!({
        "total": total,
        "offset": query.offset,
        "limit": query.limit,
        "logs": logs,
    }))
}

// ── Helpers ──────────────────────────────────────────────────────────────

fn json_response<T: Serialize>(data: &T) -> Response {
    let bytes = serde_json::to_vec(data).unwrap();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(bytes))
        .unwrap()
}

fn error_response(status: StatusCode, message: &str) -> Response {
    let body = serde_json::json!({"error": message});
    let bytes = serde_json::to_vec(&body).unwrap();
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(bytes))
        .unwrap()
}
