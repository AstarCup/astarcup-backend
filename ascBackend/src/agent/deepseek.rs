// DeepSeek（OpenAI 兼容）Chat Completions 客户端：支持工具调用（tool calls）
use serde_json::{Value, json};

use crate::config::Config;
use crate::error::AppError;

pub struct DeepSeekClient {
    http: reqwest::Client,
    base_url: String,
    model: String,
    api_key: String,
}

impl DeepSeekClient {
    pub fn new(http: reqwest::Client, cfg: &Config) -> Self {
        DeepSeekClient {
            http,
            base_url: cfg.deepseek_base_url.trim_end_matches('/').to_string(),
            model: cfg.deepseek_model.clone(),
            api_key: cfg.deepseek_api_key.clone(),
        }
    }

    pub fn enabled(&self) -> bool {
        !self.api_key.is_empty()
    }

    /// 发起一轮对话；返回 choices[0].message（含 content 或 tool_calls）
    pub async fn chat(&self, messages: &[Value], tools: &[Value]) -> Result<Value, AppError> {
        let mut body = json!({
            "model": self.model,
            "messages": messages,
            "temperature": 0.3,
        });
        if !tools.is_empty() {
            body["tools"] = json!(tools);
            body["tool_choice"] = json!("auto");
        }
        let resp = self
            .http
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::bad_gateway(format!("DeepSeek 请求失败: {e}")))?;
        let status = resp.status();
        let body: Value = resp
            .json()
            .await
            .map_err(|e| AppError::internal(format!("DeepSeek 响应解析失败: {e}")))?;
        if !status.is_success() {
            return Err(AppError::bad_gateway(format!(
                "DeepSeek 调用失败 (HTTP {status}): {body}"
            )));
        }
        let message = body["choices"][0]["message"].clone();
        if message.is_object() {
            Ok(message)
        } else {
            Err(AppError::internal(format!(
                "DeepSeek 响应缺少 message: {body}"
            )))
        }
    }
}
