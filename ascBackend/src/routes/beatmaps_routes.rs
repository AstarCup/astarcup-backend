// beatmap 工具路由（合并原 parse-beatmap、calculate-mod-stats、parse-osz、
// download-beatmap、download-blob）
//
//   POST /api/beatmaps/parse-url    解析 beatmap URL → osu! API 信息（管理员）
//   POST /api/beatmaps/mod-stats    计算指定 mod 下的难度属性（OsuNodeHelper）
//   POST /api/beatmaps/parse-osz    解析 .osz（multipart 上传或浏览器预解析 JSON）并存档
//   GET  /api/beatmaps/download?sid=&source=   图包下载代理（sayobot/nerinyan/osu）
//   GET  /api/files/{*path}         本地存储文件下载（替代原 download-blob）
use std::io::Read;

use axum::body::Body;
use axum::extract::FromRequest;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::header;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};

use crate::error::{ApiResult, AppError, ok};
use crate::state::AppState;
use crate::util::{
    ModSettings, build_mod_stats, calculate_difficulty, custom_osz_path, osu_to_beatmap_info,
    parse_beatmap_url, storage_path,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/beatmaps/parse-url", post(parse_url))
        .route("/api/beatmaps/mod-stats", post(mod_stats))
        .route("/api/beatmaps/parse-osz", post(parse_osz))
        .route("/api/beatmaps/download", get(download_beatmap))
        .route("/api/files/{*path}", get(download_file))
}

// ============ POST /api/beatmaps/parse-url ============

#[derive(serde::Deserialize)]
struct ParseUrlBody {
    url: String,
    #[serde(default)]
    osu_id: Option<String>,
}

async fn parse_url(
    State(state): State<AppState>,
    Json(body): Json<ParseUrlBody>,
) -> ApiResult<Json<Value>> {
    if let Some(osu_id) = &body.osu_id {
        let permissions = crate::routes::users_routes::permissions_json(&state, osu_id).await?;
        if !permissions["isadmin"].as_bool().unwrap_or(false) {
            return Err(AppError::forbidden("您没有权限访问选图系统"));
        }
    }
    let parsed = parse_beatmap_url(&body.url);
    if parsed.beatmap_id.is_none() && parsed.beatmapset_id.is_none() {
        return Err(AppError::bad_request("无效的osu! beatmap URL"));
    }
    let beatmaps = if let Some(bid) = parsed.beatmap_id {
        vec![crate::routes::maps_routes::fetch_beatmap_info(&state, bid).await?]
    } else {
        let bsid = parsed.beatmapset_id.unwrap();
        let maps = crate::routes::maps_routes::fetch_beatmapset_info(&state, bsid).await?;
        if maps.is_empty() {
            return Err(AppError::bad_request("该beatmapset中没有找到任何beatmap"));
        }
        maps
    };
    Ok(ok(json!(beatmaps)))
}

// ============ POST /api/beatmaps/mod-stats ============

#[derive(serde::Deserialize)]
struct ModStatsBody {
    #[serde(default)]
    beatmap: Option<Value>,
    #[serde(default)]
    mods: Option<String>,
    #[serde(default)]
    custom_settings: Option<Value>,
    #[serde(rename = "customSettings", default)]
    custom_settings_alias: Option<Value>,
    #[serde(default)]
    osu_content: Option<String>,
    #[serde(rename = "osuContent", default)]
    osu_content_alias: Option<String>,
}

async fn mod_stats(
    _state: State<AppState>,
    Json(body): Json<ModStatsBody>,
) -> ApiResult<Json<Value>> {
    let beatmap_info = body.beatmap.clone().unwrap_or(Value::Null);
    let beatmap_id = beatmap_info.get("id").and_then(|v| v.as_i64()).or_else(|| {
        body.beatmap
            .as_ref()
            .and_then(|b| b.get("id"))
            .and_then(|v| v.as_i64())
    });
    let osu_content = body.osu_content.or(body.osu_content_alias);
    if beatmap_id.is_none() && osu_content.is_none() {
        return Err(AppError::bad_request(
            "Either beatmapId or osuContent is required",
        ));
    }

    let custom = body
        .custom_settings
        .or(body.custom_settings_alias)
        .unwrap_or(Value::Null);
    let custom_mod_name = custom
        .get("customModName")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let custom_dt_rate = custom.get("customDTRate").and_then(|v| v.as_f64());
    let custom_da_settings = custom.get("customDASettings").cloned();
    let mods = body.mods.as_deref().unwrap_or("NM");

    let settings = ModSettings {
        mods,
        custom_mod_name: custom_mod_name.as_deref(),
        custom_dt_rate,
    };

    // rosu-pp 计算（失败时回退模拟值，保持容错）
    let (full_result, star_rating, aim, speed) =
        match calculate_difficulty(beatmap_id, &settings, osu_content.as_deref()).await {
            Ok(r) => {
                let sr = r.get("star_rating").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let a = r
                    .get("aim_difficulty")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let s = r
                    .get("speed_difficulty")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                (Some(r), sr, a, s)
            }
            Err(e) => {
                tracing::warn!("难度计算失败，使用模拟值: {e}");
                (None, 5.0, 3.0, 2.0)
            }
        };

    let mod_stats = build_mod_stats(
        full_result.as_ref(),
        &beatmap_info,
        &settings,
        custom_da_settings.as_ref(),
    );
    let _ = (star_rating, aim, speed);

    Ok(ok(json!({
        "modStats": mod_stats,
        "mods": mods,
        "customModName": custom_mod_name,
        "customDTRate": custom_dt_rate,
    })))
}

// ============ POST /api/beatmaps/parse-osz ============

/// 对单个难度执行 mod-stats 计算（内部复用 calculate_difficulty）
async fn compute_beatmap_mods(
    mut info: Value,
    osu_content: Option<&str>,
    selected_mods: &str,
) -> Value {
    let beatmap_id = info.get("beatmapId").and_then(|v| v.as_i64());
    let settings = ModSettings {
        mods: selected_mods,
        custom_mod_name: None,
        custom_dt_rate: None,
    };
    match calculate_difficulty(beatmap_id, &settings, osu_content).await {
        Ok(r) => {
            info["cs"] = r.get("cs").cloned().unwrap_or(info["cs"].clone());
            info["ar"] = r.get("ar").cloned().unwrap_or(info["ar"].clone());
            info["od"] = r.get("od").cloned().unwrap_or(info["od"].clone());
            info["hp"] = r.get("hp").cloned().unwrap_or(info["hp"].clone());
            info["bpm"] = r.get("bpm").cloned().unwrap_or(info["bpm"].clone());
            info["totalLength"] = r
                .get("length")
                .cloned()
                .unwrap_or(info["totalLength"].clone());
            info["maxCombo"] = json!(r.get("max_combo").and_then(|v| v.as_i64()).unwrap_or(0));
            info["starRating"] =
                json!(r.get("star_rating").and_then(|v| v.as_f64()).unwrap_or(5.0));
            info["modStats"] = build_mod_stats(Some(&r), &info, &settings, None);
        }
        Err(e) => {
            tracing::warn!(
                "mod-stats 计算失败（{}）：{e}",
                info.get("osuFilename")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
            );
            info["starRating"] = json!(5.0);
            info["maxCombo"] = json!(0);
        }
    }
    info
}

#[derive(serde::Deserialize, Default)]
struct ParseOszJsonBody {
    #[serde(default)]
    user_id: Option<String>,
    #[serde(rename = "userId", default)]
    user_id_alias: Option<String>,
    #[serde(default)]
    season: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(rename = "selectedMods", default)]
    selected_mods: Option<String>,
    #[serde(rename = "modPosition", default)]
    mod_position: Option<Value>,
    #[serde(rename = "customBeatmapId", default)]
    custom_beatmap_id: Option<Value>,
    #[serde(rename = "parsedInBrowser", default)]
    parsed_in_browser: bool,
    #[serde(rename = "beatmapInfos", default)]
    beatmap_infos: Vec<Value>,
}

async fn parse_osz(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> ApiResult<Json<Value>> {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let is_multipart = content_type.starts_with("multipart/form-data");

    let (
        file_name,
        file_size,
        mut file_bytes,
        user_id,
        season,
        category,
        selected_mods,
        mod_position,
        custom_beatmap_id,
        browser_infos,
    ) = if is_multipart {
        let http_req = axum::http::Request::builder()
            .header(header::CONTENT_TYPE, &content_type)
            .body(Body::from(body))
            .map_err(|e| AppError::internal(format!("构造请求失败: {e}")))?;
        let mut multipart = Multipart::from_request(http_req, &state)
            .await
            .map_err(|e| AppError::bad_request(format!("解析 multipart 失败: {e}")))?;
        let mut file_name = String::new();
        let mut file_bytes: Vec<u8> = Vec::new();
        let mut user_id = String::new();
        let mut season = "s1".to_string();
        let mut category = "qualification".to_string();
        let mut selected_mods = "NM".to_string();
        let mut mod_position = "1".to_string();
        let mut custom_beatmap_id: Option<String> = None;
        while let Some(field) = multipart
            .next_field()
            .await
            .map_err(|e| AppError::bad_request(format!("读取 multipart 字段失败: {e}")))?
        {
            match field.name().unwrap_or("") {
                "file" => {
                    file_name = field.file_name().unwrap_or("").to_string();
                    file_bytes = field
                        .bytes()
                        .await
                        .map_err(|e| AppError::bad_request(format!("读取文件失败: {e}")))?
                        .to_vec();
                }
                "userId" => user_id = field.text().await.unwrap_or_default(),
                "season" => {
                    let v = field.text().await.unwrap_or_default();
                    if !v.is_empty() {
                        season = v;
                    }
                }
                "category" => {
                    let v = field.text().await.unwrap_or_default();
                    if !v.is_empty() {
                        category = v;
                    }
                }
                "selectedMods" => {
                    let v = field.text().await.unwrap_or_default();
                    if !v.is_empty() {
                        selected_mods = v;
                    }
                }
                "modPosition" => {
                    let v = field.text().await.unwrap_or_default();
                    if !v.is_empty() {
                        mod_position = v;
                    }
                }
                "customBeatmapId" => {
                    let v = field.text().await.unwrap_or_default();
                    if !v.is_empty() {
                        custom_beatmap_id = Some(v);
                    }
                }
                _ => {}
            }
        }
        (
            file_name,
            file_bytes.len(),
            Some(file_bytes),
            user_id,
            season,
            category,
            selected_mods,
            mod_position,
            custom_beatmap_id,
            Vec::new(),
        )
    } else {
        let json_body: ParseOszJsonBody = serde_json::from_slice(&body)
            .map_err(|e| AppError::bad_request(format!("解析 JSON 失败: {e}")))?;
        let user_id = json_body
            .user_id
            .or(json_body.user_id_alias)
            .ok_or_else(|| AppError::bad_request("缺少必要参数：userId"))?;
        let infos = if json_body.parsed_in_browser {
            json_body.beatmap_infos
        } else {
            return Err(AppError::bad_request("缺少必要参数：beatmapInfos"));
        };
        (
            "browser-parsed.osz".to_string(),
            0,
            None,
            user_id,
            json_body.season.unwrap_or_else(|| "s1".into()),
            json_body.category.unwrap_or_else(|| "qualification".into()),
            json_body.selected_mods.unwrap_or_else(|| "NM".into()),
            json_body
                .mod_position
                .as_ref()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "1".into()),
            json_body.custom_beatmap_id.as_ref().map(|v| v.to_string()),
            infos,
        )
    };

    // 权限校验（选图系统 = 管理员）
    let permissions = crate::routes::users_routes::permissions_json(&state, &user_id).await?;
    if !permissions["isadmin"].as_bool().unwrap_or(false) {
        return Err(AppError::forbidden("您没有权限访问选图系统"));
    }

    let selected_mods_c = selected_mods.clone();
    let season_c = season.clone();
    let category_c = category.clone();
    let mod_position_i: i64 = mod_position.parse().unwrap_or(1);

    // 逐难度计算并合并
    let mut with_mods: Vec<Value> = Vec::new();
    if !browser_infos.is_empty() {
        for info in browser_infos {
            let osu_content = info
                .get("osuContent")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let mut merged = info.clone();
            if let Some(content) = &osu_content {
                merged = compute_beatmap_mods(merged, Some(content), &selected_mods_c).await;
            } else {
                merged["starRating"] = json!(5.0);
                merged["maxCombo"] = json!(0);
            }
            merged["osuContent"] = Value::Null;
            with_mods.push(merged);
        }
    } else {
        // multipart 分支：解压 .osz
        let file_bytes = file_bytes
            .take()
            .ok_or_else(|| AppError::bad_request("缺少文件"))?;
        let name_lower = file_name.to_lowercase();
        if !name_lower.ends_with(".osz") {
            return Err(AppError::bad_request("只支持.osz文件格式"));
        }
        if file_size > 50 * 1024 * 1024 {
            return Err(AppError::bad_request("文件大小不能超过50MB"));
        }
        let reader = std::io::Cursor::new(&file_bytes);
        let mut zip = zip::ZipArchive::new(reader)
            .map_err(|e| AppError::bad_request(format!("解析osz文件失败: {e}")))?;
        // 收集 .osu 文件名
        let mut osu_names: Vec<String> = Vec::new();
        for i in 0..zip.len() {
            let f = zip
                .by_index_raw(i)
                .map_err(|e| AppError::bad_request(format!("读取zip失败: {e}")))?;
            let name = f.name().to_string();
            if name.to_lowercase().ends_with(".osu") {
                osu_names.push(name);
            }
        }
        if osu_names.is_empty() {
            return Err(AppError::bad_request("在osz文件中未找到.osu文件"));
        }
        let mut parsed_infos: Vec<(String, Value)> = Vec::new();
        for (idx, name) in osu_names.iter().enumerate() {
            let mut f = zip
                .by_name(name)
                .map_err(|e| AppError::bad_request(format!("读取 {name} 失败: {e}")))?;
            let mut content = String::new();
            f.read_to_string(&mut content)
                .map_err(|e| AppError::bad_request(format!("读取.osu内容失败: {e}")))?;
            let mut info = osu_to_beatmap_info(&content, name, idx);
            info["osuContent"] = json!(content);
            parsed_infos.push((name.clone(), info));
        }
        if parsed_infos.is_empty() {
            return Err(AppError::bad_request("无法解析任何.osu文件"));
        }
        // 按难度名排序
        parsed_infos.sort_by(|a, b| {
            let av = a.1["version"].as_str().unwrap_or("");
            let bv = b.1["version"].as_str().unwrap_or("");
            av.cmp(bv)
        });
        for (_, mut info) in parsed_infos {
            let content = info
                .get("osuContent")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            if let Some(c) = &content {
                info = compute_beatmap_mods(info, Some(c), &selected_mods_c).await;
            } else {
                info["starRating"] = json!(5.0);
                info["maxCombo"] = json!(0);
            }
            with_mods.push(info);
        }
    }
    with_mods.sort_by(|a, b| {
        let av = a["version"].as_str().unwrap_or("");
        let bv = b["version"].as_str().unwrap_or("");
        av.cmp(bv)
    });

    // 生成存储路径
    let final_beatmap_id: Option<i64> = custom_beatmap_id
        .as_deref()
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|v| *v < 0)
        .or_else(|| {
            with_mods
                .first()
                .and_then(|i| i.get("beatmapId"))
                .and_then(|v| v.as_i64())
                .filter(|v| *v > 0)
        });
    let blob_path = custom_osz_path(
        &season_c,
        &category_c,
        &selected_mods_c,
        mod_position_i,
        final_beatmap_id,
    );

    // 保存 .osz 到本地存储
    let mut blob_url = String::new();
    if let Some(bytes) = &file_bytes {
        let abs = storage_path(&state.config, &blob_path)?;
        if let Some(parent) = abs.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(&abs, bytes) {
            Ok(_) => blob_url = format!("/api/files{}", blob_path),
            Err(e) => tracing::error!("保存osz失败: {e}"),
        }
    }

    let total = with_mods.len();
    Ok(crate::error::ok_msg(
        "osz文件解析成功",
        json!({
            "beatmapInfos": with_mods,
            "fileInfo": {
                "originalName": file_name,
                "size": file_size,
                "blobPath": blob_path,
                "blobUrl": blob_url,
                "uploaded": !blob_url.is_empty(),
            },
            "totalDifficulties": total,
        }),
    ))
}

// ============ GET /api/beatmaps/download ============

#[derive(serde::Deserialize)]
struct DownloadQuery {
    sid: Option<String>,
    source: Option<String>,
}

async fn download_beatmap(
    Query(q): Query<DownloadQuery>,
    headers: axum::http::HeaderMap,
) -> Result<Response, AppError> {
    let sid = q
        .sid
        .ok_or_else(|| AppError::bad_request("Missing sid parameter"))?;
    let source = q.source.unwrap_or_else(|| "sayobot".into());
    let download_url = match source.as_str() {
        "osu" => format!("https://osu.ppy.sh/beatmapsets/{sid}/download"),
        "nerinyan" => format!("https://api.nerinyan.moe/d/{sid}"),
        _ => format!("https://dl.sayobot.cn/beatmaps/download/full/{sid}"),
    };
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120.0.0.0 Safari/537.36")
        .to_string();

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| AppError::internal(format!("构建下载客户端失败: {e}")))?;
    let resp = client
        .get(&download_url)
        .header(header::USER_AGENT, &user_agent)
        .header(header::REFERER, "https://www.rino.ink/")
        .header(header::ACCEPT, "*/*")
        .send()
        .await
        .map_err(|e| AppError::internal(format!("下载代理请求失败: {e}")))?;
    if !resp.status().is_success() {
        return Err(AppError::new(
            axum::http::StatusCode::from_u16(resp.status().as_u16())
                .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
            format!("Download failed: {}", resp.status()),
        ));
    }
    let filename = resp
        .headers()
        .get(header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .and_then(|cd| {
            let re = regex::Regex::new(r#"filename[^;=\n]*=(([']["']).*?\2|[^;\n]*)"#).ok()?;
            re.captures(cd)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().replace(['\'', '"'], ""))
        })
        .unwrap_or_else(|| format!("beatmap_{sid}.osz"));

    let stream = resp.bytes_stream();
    Ok(Response::builder()
        .status(200)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from_stream(stream))
        .map_err(|e| AppError::internal(format!("构建下载响应失败: {e}")))?)
}

// ============ GET /api/files/{*path} ============

async fn download_file(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> Result<Response, AppError> {
    let abs = storage_path(&state.config, &path)?;
    let file_name = abs
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();
    if !abs.exists() {
        return Err(AppError::not_found("文件不存在"));
    }
    let bytes = tokio::fs::read(&abs)
        .await
        .map_err(|e| AppError::internal(format!("读取文件失败: {e}")))?;
    let mut filename = file_name;
    if !filename.to_lowercase().ends_with(".osz") {
        filename = format!("{filename}.osz");
    }
    let mut resp = Response::builder()
        .status(200)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(bytes))
        .map_err(|e| AppError::internal(format!("构建响应失败: {e}")))?;
    resp.headers_mut().remove(header::CONTENT_LENGTH);
    Ok(resp)
}
