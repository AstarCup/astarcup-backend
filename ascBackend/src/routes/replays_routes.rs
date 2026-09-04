// 回放文件路由（合并原 upload-replay、download-all-replays、uploaded-users）
//
//   POST   /api/replays                          上传回放（multipart：file/filename/userId，管理员）
//   DELETE /api/replays                          删除回放（JSON：filename/userId，管理员）
//   GET    /api/replays/archive?season=&category=  打包下载（zip 按 mod 位分目录）
//   GET    /api/replays/uploaded?season=&category= 上传状态（mapKey → 用户名列表，管理员）
use std::io::Write;

use axum::body::Body;
use axum::extract::{Multipart, Query, State};
use axum::http::header;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};

use crate::error::{ApiResult, AppError, ok};
use crate::state::AppState;
use crate::util::storage_path;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/replays", post(upload_replay).delete(delete_replay))
        .route("/api/replays/archive", get(archive))
        .route("/api/replays/uploaded", get(uploaded))
}

// ============ POST /api/replays ============

async fn upload_replay(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> ApiResult<Json<Value>> {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut filename = String::new();
    let mut user_id = String::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request(format!("解析 multipart 失败: {e}")))?
    {
        match field.name().unwrap_or("") {
            "file" => {
                file_bytes = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| AppError::bad_request(format!("读取文件失败: {e}")))?
                        .to_vec(),
                );
            }
            "filename" => filename = field.text().await.unwrap_or_default(),
            "userId" => user_id = field.text().await.unwrap_or_default(),
            _ => {}
        }
    }
    if file_bytes.is_none() || filename.is_empty() || user_id.is_empty() {
        return Err(AppError::bad_request("参数缺失"));
    }
    // 回放上传权限 = 管理员
    let permissions = crate::routes::users_routes::permissions_json(&state, &user_id).await?;
    if !permissions["isadmin"].as_bool().unwrap_or(false) {
        return Err(AppError::forbidden("无权限"));
    }

    let abs = storage_path(&state.config, &filename)?;
    if let Some(parent) = abs.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    tokio::fs::write(&abs, file_bytes.unwrap())
        .await
        .map_err(|e| AppError::internal(format!("上传失败: {e}")))?;
    Ok(Json(
        json!({ "success": true, "data": { "url": format!("/api/files/{filename}") } }),
    ))
}

// ============ DELETE /api/replays ============

#[derive(serde::Deserialize)]
struct DeleteReplayBody {
    #[serde(default)]
    filename: Option<String>,
    #[serde(default)]
    user_id: Option<String>,
    #[serde(rename = "userId", default)]
    user_id_alias: Option<String>,
}

async fn delete_replay(
    State(state): State<AppState>,
    user: crate::auth::SessionUser,
    Json(body): Json<DeleteReplayBody>,
) -> ApiResult<Json<Value>> {
    let filename = body
        .filename
        .ok_or_else(|| AppError::bad_request("参数缺失"))?;
    let operator = body
        .user_id
        .or(body.user_id_alias)
        .unwrap_or_else(|| user.osu_id.clone());
    let permissions = crate::routes::users_routes::permissions_json(&state, &operator).await?;
    if !permissions["isadmin"].as_bool().unwrap_or(false) {
        return Err(AppError::forbidden("无权限"));
    }
    let abs = storage_path(&state.config, &filename)?;
    match tokio::fs::remove_file(&abs).await {
        Ok(_) => Ok(Json(json!({ "success": true, "data": null }))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(AppError::not_found("删除失败，文件不存在"))
        }
        Err(e) => Err(AppError::internal(format!("删除失败: {e}"))),
    }
}

// ============ GET /api/replays/archive ============

#[derive(serde::Deserialize)]
struct ArchiveQuery {
    season: Option<String>,
    category: Option<String>,
}

async fn archive(
    State(state): State<AppState>,
    Query(q): Query<ArchiveQuery>,
) -> Result<Response, AppError> {
    let season = q.season.unwrap_or_else(|| "s1".into());
    let category = q.category.unwrap_or_else(|| "qualification".into());
    let dir_rel = format!("{season}/{category}");
    let abs_dir = storage_path(&state.config, &dir_rel)?;

    // filename 格式: modPosition_bid_userId_username.osr
    let mut files_by_mod: std::collections::BTreeMap<String, Vec<(String, Vec<u8>)>> =
        Default::default();
    if abs_dir.exists() {
        let mut entries = tokio::fs::read_dir(&abs_dir)
            .await
            .map_err(|e| AppError::internal(format!("读取目录失败: {e}")))?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| AppError::internal(format!("读取目录失败: {e}")))?
        {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.to_lowercase().ends_with(".osr") {
                continue;
            }
            let parts: Vec<&str> = name.split('_').collect();
            if parts.len() < 4 {
                continue;
            }
            let mod_position = parts[0].to_string();
            let bid = parts[1].to_string();
            let username = parts[3].trim_end_matches(".osr").to_string();
            let bytes = tokio::fs::read(entry.path())
                .await
                .map_err(|e| AppError::internal(format!("读取回放失败: {e}")))?;
            let new_name = format!("{mod_position}_{username}_{bid}.osr");
            files_by_mod
                .entry(mod_position)
                .or_default()
                .push((new_name, bytes));
        }
    }
    if files_by_mod.is_empty() {
        return Err(AppError::not_found("该赛季和类别下没有已上传的回放文件"));
    }

    // 生成 zip，按 mod 位分目录
    let buf = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, AppError> {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default();
        for (mod_position, files) in &files_by_mod {
            for (name, content) in files {
                let entry = format!("{mod_position}/{name}");
                zip.start_file(entry, options)
                    .map_err(|e| AppError::internal(format!("压缩失败: {e}")))?;
                zip.write_all(content)
                    .map_err(|e| AppError::internal(format!("压缩失败: {e}")))?;
            }
        }
        let writer = zip
            .finish()
            .map_err(|e| AppError::internal(format!("生成zip失败: {e}")))?;
        Ok(writer.into_inner())
    })
    .await
    .map_err(|e| AppError::internal(format!("任务调度失败: {e}")))??;

    Ok(Response::builder()
        .status(200)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{season}_{category}_replays.zip\""),
        )
        .body(Body::from(buf))
        .map_err(|e| AppError::internal(format!("构建响应失败: {e}")))?)
}

// ============ GET /api/replays/uploaded ============

async fn uploaded(
    State(state): State<AppState>,
    _admin: crate::auth::AdminUser,
    Query(q): Query<ArchiveQuery>,
) -> ApiResult<Json<Value>> {
    let season = q.season.unwrap_or_else(|| "s1".into());
    let category = q.category.unwrap_or_else(|| "qualification".into());
    let abs_dir = storage_path(&state.config, &format!("{season}/{category}"))?;
    let mut uploaded_users: serde_json::Map<String, Value> = serde_json::Map::new();
    if abs_dir.exists() {
        let mut entries = tokio::fs::read_dir(&abs_dir)
            .await
            .map_err(|e| AppError::internal(format!("读取目录失败: {e}")))?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| AppError::internal(format!("读取目录失败: {e}")))?
        {
            let name = entry.file_name().to_string_lossy().to_string();
            let parts: Vec<&str> = name.split('_').collect();
            if parts.len() < 4 {
                continue;
            }
            let bid = parts[1];
            let username = parts[3].trim_end_matches(".osr");
            let key = format!("{season}/{category}/{bid}");
            let list = uploaded_users.entry(key).or_insert_with(|| json!([]));
            if let Some(arr) = list.as_array_mut() {
                if !arr.iter().any(|v| v.as_str() == Some(username)) {
                    arr.push(json!(username));
                }
            }
        }
    }
    Ok(ok(Value::Object(uploaded_users)))
}
