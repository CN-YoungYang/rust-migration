use crate::{
    crypto,
    error::{AppError, Result},
    security::validate_public_http_url_resolved,
    AppState,
};
use axum::{
    body::Bytes,
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Extension,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// CSV 导出的账户记录
#[derive(Serialize)]
struct ExportAccountRecord {
    name: String,
    #[serde(rename = "siteType")]
    site_type: String,
    #[serde(rename = "baseUrl")]
    base_url: String,
    #[serde(rename = "userId")]
    user_id: String,
    #[serde(rename = "authType")]
    auth_type: String,
    #[serde(rename = "accessToken")]
    access_token: String,
    cookie: String,
    #[serde(rename = "customCheckinUrl")]
    custom_checkin_url: String,
    enabled: String,
    #[serde(rename = "retryEnabled")]
    retry_enabled: String,
    note: String,
}

/// CSV 导入的账户记录
#[derive(Deserialize)]
struct ImportAccountRecord {
    name: String,
    #[serde(rename = "siteType")]
    site_type: String,
    #[serde(rename = "baseUrl")]
    base_url: String,
    #[serde(rename = "userId")]
    user_id: Option<String>,
    #[serde(rename = "authType")]
    auth_type: String,
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    cookie: Option<String>,
    #[serde(rename = "customCheckinUrl")]
    custom_checkin_url: Option<String>,
    enabled: Option<String>,
    #[serde(rename = "retryEnabled")]
    retry_enabled: Option<String>,
    note: Option<String>,
}

#[derive(Serialize)]
pub struct ImportResult {
    /// 成功导入的账户数
    success: usize,
    /// 失败的行数
    failed: usize,
    /// 错误信息列表
    errors: Vec<String>,
}

async fn validate_import_record(
    record: &ImportAccountRecord,
    line_num: usize,
) -> std::result::Result<(), String> {
    let site_type = record.site_type.trim();
    let auth_type = record.auth_type.trim();

    if record.name.trim().is_empty() {
        return Err(format!("第 {} 行：账户名称不能为空", line_num));
    }
    if record.base_url.trim().is_empty() {
        return Err(format!("第 {} 行：站点地址不能为空", line_num));
    }
    validate_public_http_url_resolved(record.base_url.trim(), "站点地址")
        .await
        .map_err(|e| format!("第 {} 行：{}", line_num, e))?;

    if !["new-api", "anyrouter", "x666"].contains(&site_type) {
        return Err(format!(
            "第 {} 行：不支持的站点类型 {}",
            line_num, site_type
        ));
    }
    if !["access_token", "cookie"].contains(&auth_type) {
        return Err(format!(
            "第 {} 行：不支持的认证方式 {}",
            line_num, auth_type
        ));
    }

    let user_id = record.user_id.as_deref().unwrap_or("").trim();
    let access_token = record.access_token.as_deref().unwrap_or("").trim();
    let cookie = record.cookie.as_deref().unwrap_or("").trim();

    if site_type == "anyrouter" && user_id.is_empty() {
        return Err(format!("第 {} 行：AnyRouter 必须填写 userId", line_num));
    }
    if (site_type == "anyrouter" || site_type == "x666") && cookie.is_empty() {
        return Err(format!("第 {} 行：{} 必须填写 cookie", line_num, site_type));
    }
    if site_type == "new-api" && auth_type == "access_token" && access_token.is_empty() {
        return Err(format!(
            "第 {} 行：认证方式为 access_token 时必须填写 accessToken",
            line_num
        ));
    }

    if let Some(custom_url) = record.custom_checkin_url.as_deref().map(str::trim) {
        if custom_url.starts_with("http://") || custom_url.starts_with("https://") {
            validate_public_http_url_resolved(custom_url, "自定义签到地址")
                .await
                .map_err(|e| format!("第 {} 行：{}", line_num, e))?;
        }
    }

    Ok(())
}

/// GET /api/accounts/export - 导出账户为 CSV
pub async fn export_accounts(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::models::AppUser>,
) -> Result<Response> {
    let is_admin = user.role == "ADMIN" || user.role == "SUPER_ADMIN";

    // 查询账户（管理员看全部，普通用户只看自己的）
    let accounts = if is_admin {
        crate::db::list_enabled_accounts(&state.db).await?
    } else {
        crate::db::list_accounts_filtered(
            &state.db,
            &crate::db::AccountFilter {
                owner_id: Some(user.id.clone()),
                site_type: None,
                enabled: None,
                last_status: None,
                keyword: None,
                limit: 10000,
                offset: 0,
            },
        )
        .await?
    };

    // 需要查询完整账户信息（包含加密字段）
    let account_ids: Vec<String> = accounts.iter().map(|a| a.id.clone()).collect();
    let full_accounts = crate::db::find_accounts_by_ids(&state.db, &account_ids).await?;

    // 构建 CSV
    let mut wtr = csv::Writer::from_writer(vec![]);

    for account in accounts.iter() {
        let full_account = match full_accounts.get(&account.id) {
            Some(a) => a,
            None => continue,
        };

        // 解密凭证
        let access_token = full_account
            .access_token_enc
            .as_ref()
            .and_then(|enc| crypto::decrypt(enc).ok())
            .unwrap_or_default();

        let cookie = full_account
            .cookie_enc
            .as_ref()
            .and_then(|enc| crypto::decrypt(enc).ok())
            .unwrap_or_default();

        let record = ExportAccountRecord {
            name: account.name.clone(),
            site_type: account.site_type.clone(),
            base_url: account.base_url.clone(),
            user_id: account.user_id.clone().unwrap_or_default(),
            auth_type: account.auth_type.clone(),
            access_token,
            cookie,
            custom_checkin_url: account.custom_checkin_url.clone().unwrap_or_default(),
            enabled: if account.enabled { "true" } else { "false" }.to_string(),
            retry_enabled: if account.retry_enabled {
                "true"
            } else {
                "false"
            }
            .to_string(),
            note: account.note.clone().unwrap_or_default(),
        };

        wtr.serialize(record)?;
    }

    let csv_data = wtr
        .into_inner()
        .map_err(|e| AppError::Internal(format!("生成 CSV 失败: {}", e)))?;

    // 返回 CSV 文件
    let filename = format!(
        "ai-hub-accounts-{}.csv",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    );

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(axum::body::Body::from(csv_data))
        .map_err(|e| AppError::Internal(format!("构建导出响应失败: {}", e)))?
        .into_response())
}

/// POST /api/accounts/import - 从 CSV 导入账户
pub async fn import_accounts(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::models::AppUser>,
    body: Bytes,
) -> Result<axum::Json<serde_json::Value>> {
    // 解析 CSV
    let mut rdr = csv::Reader::from_reader(body.as_ref());
    let mut success_count = 0;
    let mut failed_count = 0;
    let mut errors = Vec::new();

    for (idx, result) in rdr.deserialize().enumerate() {
        let line_num = idx + 2; // CSV 行号（包含 header）

        let record: ImportAccountRecord = match result {
            Ok(r) => r,
            Err(e) => {
                failed_count += 1;
                errors.push(format!("第 {} 行：CSV 格式错误 - {}", line_num, e));
                continue;
            }
        };

        if let Err(e) = validate_import_record(&record, line_num).await {
            failed_count += 1;
            errors.push(e);
            continue;
        }

        // 加密凭证
        let access_token_enc = record
            .access_token
            .filter(|s| !s.trim().is_empty())
            .map(|token| crypto::encrypt(&token))
            .transpose()
            .map_err(|e| AppError::Internal(format!("加密失败: {}", e)))?;

        let cookie_enc = record
            .cookie
            .filter(|s| !s.trim().is_empty())
            .map(|cookie| crypto::encrypt(&cookie))
            .transpose()
            .map_err(|e| AppError::Internal(format!("加密失败: {}", e)))?;

        // 创建账户
        let enabled = record
            .enabled
            .as_ref()
            .map(|s| s.trim().eq_ignore_ascii_case("true"))
            .unwrap_or(true);

        let retry_enabled = record
            .retry_enabled
            .as_ref()
            .map(|s| s.trim().eq_ignore_ascii_case("true"))
            .unwrap_or(true);

        let create_req = crate::db::CreateAccountRequest {
            name: record.name.trim().to_string(),
            site_type: record.site_type.trim().to_string(),
            base_url: record.base_url.trim().to_string(),
            user_id: record.user_id.filter(|s| !s.trim().is_empty()),
            owner_id: user.id.clone(),
            auth_type: if record.site_type.trim() == "anyrouter"
                || record.site_type.trim() == "x666"
            {
                "cookie".to_string()
            } else {
                record.auth_type.trim().to_string()
            },
            access_token_enc,
            cookie_enc,
            custom_checkin_url: record.custom_checkin_url.filter(|s| !s.trim().is_empty()),
            enabled,
            retry_enabled,
            note: record.note.filter(|s| !s.trim().is_empty()),
        };

        match crate::db::create_account(&state.db, &create_req).await {
            Ok(_) => success_count += 1,
            Err(e) => {
                failed_count += 1;
                errors.push(format!("第 {} 行：创建账户失败 - {}", line_num, e));
            }
        }
    }

    Ok(crate::routes::data(ImportResult {
        success: success_count,
        failed: failed_count,
        errors,
    }))
}
