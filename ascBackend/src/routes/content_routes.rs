// 内容敏感词检测路由（原 content-detect）
//
//   POST /api/content/check {text}
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{Value, json};

use crate::error::{ApiResult, AppError, ok};
use crate::state::AppState;
use crate::util::prohibited_words;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/content/check", post(check))
}

#[derive(serde::Deserialize)]
struct CheckBody {
    #[serde(default)]
    text: Option<String>,
}

async fn check(
    State(state): State<AppState>,
    Json(body): Json<CheckBody>,
) -> ApiResult<Json<Value>> {
    let text = body
        .text
        .ok_or_else(|| AppError::bad_request("Text is required"))?;
    let words = prohibited_words(&state.config.vocab_dir);
    let lower = text.to_lowercase();
    let triggered: Vec<&str> = words
        .iter()
        .filter(|w| lower.contains(&w.to_lowercase()))
        .map(|w| w.as_str())
        .collect();
    Ok(ok(json!({
        "hasIllegal": !triggered.is_empty(),
        "triggeredWords": triggered,
        "text": text,
    })))
}
