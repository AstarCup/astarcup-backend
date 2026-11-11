// SignalR JSON 协议（ASP.NET Core）最小客户端
//
// 用于连接 osu-server-spectator 的 referee hub（wss://spectator.ppy.sh/referee），
// 命令/事件与 applearon/apl-ref 一致（源：IRefereeHubServer.cs / IRefereeHubClient.cs）。
//
// 协议要点（JSON transport）：
//   - 传输帧为 JSON 对象，以 0x1E(Record Separator) 分隔；
//   - 握手：发送 {"protocol":"json","version":1}，应答 {} 表示成功；
//   - type 1 Invocation：客户端→服务端带 invocationId；服务端→客户端为事件推送；
//   - type 3 Completion：带 invocationId 的调用回执（result 或 error）；
//   - type 6 Ping / type 7 Close。
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, broadcast, oneshot};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, client_async_tls_with_config};

use crate::auth::url_encode;

const ARG_SEP: char = '\u{001e}';
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const PING_INTERVAL: Duration = Duration::from_secs(10);

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsSink = futures_util::stream::SplitSink<WsStream, Message>;

/// 一条 SignalR 连接：发起调用 + 等待回执；事件经 broadcast 转发。
pub struct SignalRConnection {
    sink: Arc<Mutex<WsSink>>,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<Result<Value, String>>>>>,
    next_id: AtomicU64,
}

impl SignalRConnection {
    /// 调用 hub 方法，等待 Completion 回执（不做超时控制，由调用方决定）。
    pub async fn invoke(&self, target: &str, args: Vec<Value>) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed).to_string();
        let (reply, rx) = oneshot::channel();
        self.pending.lock().await.insert(id.clone(), reply);

        let frame = json!({"type": 1, "target": target, "arguments": args, "invocationId": id});
        let text = format!("{}{}", frame, ARG_SEP);
        {
            let mut sink = self.sink.lock().await;
            if sink.send(Message::Text(text.into())).await.is_err() {
                self.pending.lock().await.remove(&id);
                return Err("连接已断开".to_string());
            }
        }
        rx.await.map_err(|_| "连接已断开".to_string())?
    }

    async fn fail_all_pending(&self, reason: &str) {
        let pending = std::mem::take(&mut *self.pending.lock().await);
        for (_, tx) in pending {
            let _ = tx.send(Err(reason.to_string()));
        }
    }

    /// 主动关闭连接（读写循环会随之退出）
    pub async fn close(&self) {
        let mut sink = self.sink.lock().await;
        let _ = sink.send(Message::Close(None)).await;
    }
}

/// 建立到 referee hub 的 SignalR 连接。
/// 返回 (连接句柄, 读写任务句柄)；任务在连接断开时结束，并让所有等待中的调用失败。
pub async fn connect(
    http_base: &str, // https://spectator.ppy.sh
    hub_path: &str,  // /referee
    token: &str,
    events: broadcast::Sender<(String, Value)>,
) -> Result<(Arc<SignalRConnection>, tokio::task::JoinHandle<()>), String> {
    // 1. negotiate 换取 connectionToken
    let negotiate_url = format!("{http_base}{hub_path}/negotiate?negotiateVersion=1");
    // reqwest 自动识别 HTTP(S)_PROXY 环境变量，代理下同样可用
    let client = reqwest::Client::new();
    let resp = tokio::time::timeout(
        CONNECT_TIMEOUT,
        client.post(&negotiate_url).bearer_auth(token).send(),
    )
    .await
    .map_err(|_| {
        "negotiate 超时（网络不可达？可设置 HTTPS_PROXY 代理或 SPECTATOR_BASE 覆盖地址）"
            .to_string()
    })?
    .map_err(|e| format!("negotiate 请求失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("negotiate 被拒绝: HTTP {}", resp.status()));
    }
    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("negotiate 响应解析失败: {e}"))?;
    let conn_token = body["connectionToken"]
        .as_str()
        .or_else(|| body["token"].as_str())
        .ok_or_else(|| "negotiate 响应缺少 connectionToken".to_string())?;

    // 2. WebSocket 连接（SignalR 对 WS 同时接受 access_token 查询参数与 Authorization 头）
    let ws_base = http_base
        .replacen("https://", "wss://", 1)
        .replacen("http://", "ws://", 1);
    let ws_url = format!(
        "{ws_base}{hub_path}?id={}&access_token={}",
        url_encode(conn_token),
        url_encode(token)
    );
    let mut request = ws_url
        .as_str()
        .into_client_request()
        .map_err(|e| format!("构造 WebSocket 请求失败: {e}"))?;
    if let Ok(v) = HeaderValue::from_str(&format!("Bearer {token}")) {
        request.headers_mut().insert("Authorization", v);
    }
    if !ws_url.starts_with("wss://") {
        return Err("仅支持 wss://（SPECTATOR_BASE 请用 https:// 开头）".to_string());
    }
    // 先建 TCP（支持 HTTP 代理 CONNECT 隧道），再在其上做 TLS + WS 握手
    let host_port = ws_url
        .trim_start_matches("wss://")
        .split(['/', '?'])
        .next()
        .unwrap_or("")
        .to_string();
    let (host, port) = match host_port.rsplit_once(':') {
        Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) => {
            (h.to_string(), p.parse::<u16>().unwrap_or(443))
        }
        _ => (host_port, 443u16),
    };
    let tcp = open_tcp(&host, port).await?;
    let (ws, _) = tokio::time::timeout(
        CONNECT_TIMEOUT,
        client_async_tls_with_config(request, tcp, None, None),
    )
    .await
    .map_err(|_| "WebSocket 握手超时".to_string())?
    .map_err(|e| format!("WebSocket 连接失败: {e}"))?;

    // 3. SignalR 握手
    let mut ws = ws;
    let hello = format!("{}{}", json!({"protocol": "json", "version": 1}), ARG_SEP);
    ws.send(Message::Text(hello.into()))
        .await
        .map_err(|e| format!("发送握手失败: {e}"))?;
    let handshake = tokio::time::timeout(HANDSHAKE_TIMEOUT, ws.next())
        .await
        .map_err(|_| "握手超时".to_string())?;
    let hs_text = match handshake {
        Some(Ok(Message::Text(t))) => t.to_string(),
        other => return Err(format!("握手应答异常: {other:?}")),
    };
    let hs_body = hs_text.split(ARG_SEP).next().unwrap_or("").to_string();
    let hs_json: Value = serde_json::from_str(&hs_body).unwrap_or(Value::Null);
    if let Some(err) = hs_json["error"].as_str() {
        return Err(format!("握手被拒绝: {err}"));
    }

    // 4. 启动读写循环
    let (sink, stream) = ws.split();
    let conn = Arc::new(SignalRConnection {
        sink: Arc::new(Mutex::new(sink)),
        pending: Arc::new(Mutex::new(HashMap::new())),
        next_id: AtomicU64::new(1),
    });
    let task_conn = conn.clone();
    let task = tokio::spawn(async move {
        run_loop(stream, task_conn, events).await;
    });
    Ok((conn, task))
}

/// 从环境变量读取 HTTP 代理（HTTPS_PROXY / ALL_PROXY 等）。
/// reqwest 会自动使用这些变量；WebSocket 侧由 open_tcp 手动走 CONNECT 隧道。
fn proxy_from_env() -> Option<String> {
    for key in [
        "ALL_PROXY",
        "all_proxy",
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
    ] {
        if let Ok(v) = std::env::var(key) {
            let v = v.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// 解析代理地址 `http://host:port`（省略 scheme 也接受；默认端口 80）
fn parse_proxy(url: &str) -> Result<(String, u16), String> {
    if url.starts_with("socks") {
        return Err("暂不支持 socks5 代理，请改用 HTTP 代理（支持 CONNECT）".to_string());
    }
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);
    match rest.rsplit_once(':') {
        Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) && !h.is_empty() => {
            Ok((h.to_string(), p.parse::<u16>().map_err(|_| "代理端口无效")?))
        }
        _ => Ok((rest.trim_end_matches('/').to_string(), 80)),
    }
}

/// 建立到目标 host:port 的 TCP：无代理直连；有代理则先 CONNECT 隧道。
async fn open_tcp(host: &str, port: u16) -> Result<TcpStream, String> {
    let Some(proxy_url) = proxy_from_env() else {
        return TcpStream::connect((host, port))
            .await
            .map_err(|e| format!("TCP 连接 {host}:{port} 失败: {e}"));
    };
    let (ph, pp) = parse_proxy(&proxy_url)?;
    let mut stream = TcpStream::connect((ph.as_str(), pp))
        .await
        .map_err(|e| format!("代理 {ph}:{pp} 连接失败: {e}"))?;
    let req = format!(
        "CONNECT {host}:{port} HTTP/1.1\r\nHost: {host}:{port}\r\nProxy-Connection: Keep-Alive\r\n\r\n"
    );
    stream
        .write_all(req.as_bytes())
        .await
        .map_err(|e| format!("代理 CONNECT 写入失败: {e}"))?;
    // 读到响应头结束（\r\n\r\n）为止
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = [0u8; 512];
    loop {
        let n = stream
            .read(&mut tmp)
            .await
            .map_err(|e| format!("代理 CONNECT 读取失败: {e}"))?;
        if n == 0 {
            return Err("代理在 CONNECT 阶段关闭了连接".to_string());
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
        if buf.len() > 8192 {
            return Err("代理 CONNECT 响应异常".to_string());
        }
    }
    let head = String::from_utf8_lossy(&buf);
    if !head.starts_with("HTTP/1.") || !head.contains(" 200") {
        let first = head.lines().next().unwrap_or("");
        return Err(format!("代理 CONNECT 被拒绝: {first}"));
    }
    Ok(stream)
}

async fn run_loop(
    mut stream: futures_util::stream::SplitStream<WsStream>,
    conn: Arc<SignalRConnection>,
    events: broadcast::Sender<(String, Value)>,
) {
    let mut ping = tokio::time::interval(PING_INTERVAL);
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = ping.tick() => {
                let mut sink = conn.sink.lock().await;
                if sink.send(Message::Text(format!("{}{}", json!({"type": 6}), ARG_SEP).into())).await.is_err() {
                    break;
                }
            }
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        for frame in text.split(ARG_SEP) {
                            if frame.is_empty() {
                                continue;
                            }
                            let Ok(v) = serde_json::from_str::<Value>(frame) else {
                                continue;
                            };
                            if !dispatch(&conn, &events, &v).await {
                                conn.close().await;
                                conn.fail_all_pending("服务器已关闭连接").await;
                                return;
                            }
                        }
                    }
                    Some(Ok(Message::Ping(p))) => {
                        let mut sink = conn.sink.lock().await;
                        let _ = sink.send(Message::Pong(p)).await;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) | None => break,
                }
            }
        }
    }
    conn.fail_all_pending("连接已断开").await;
}

/// 处理一帧服务端消息；返回 false 表示连接应结束（type 7 Close）。
async fn dispatch(
    conn: &SignalRConnection,
    events: &broadcast::Sender<(String, Value)>,
    v: &Value,
) -> bool {
    match v["type"].as_i64() {
        // Invocation：服务端→客户端 = 事件推送
        Some(1) => {
            let Some(target) = v["target"].as_str() else {
                return true;
            };
            let args = v["arguments"].as_array().cloned().unwrap_or_default();
            let data = args.first().cloned().unwrap_or(Value::Null);
            let _ = events.send((target.to_string(), data));
        }
        // Completion：调用回执
        Some(3) => {
            let Some(id) = v["invocationId"].as_str() else {
                return true;
            };
            if let Some(tx) = conn.pending.lock().await.remove(id) {
                let result = if let Some(err) = v["error"].as_str() {
                    Err(err.to_string())
                } else {
                    Ok(v.get("result").cloned().unwrap_or(Value::Null))
                };
                let _ = tx.send(result);
            }
        }
        // Close
        Some(7) => return false,
        _ => {}
    }
    true
}
