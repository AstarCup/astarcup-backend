//! 出站代理探测。
//!
//! osu! 的 referee hub 在 `wss://spectator.ppy.sh/referee`，部分网络（含本机）直连不可达，
//! 必须经本地代理（Clash/V2Ray 等）转发。浏览器与 Electron 会自动使用 Windows 系统代理，
//! 而 Rust 后端默认只认 `HTTPS_PROXY` 等环境变量，于是裁判控制台会"连不上服务器"。
//!
//! 这里统一探测：环境变量优先（`SPECTATOR_PROXY` / `HTTPS_PROXY` / `ALL_PROXY` …），
//! 其次读 Windows 的 WinINET 系统代理设置（注册表）。
//! 探测结果进程内缓存一次。

use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    /// 形如 `http://127.0.0.1:7890`
    pub url: String,
    /// 不走代理的主机列表（reqwest 的 NO_PROXY 语法，逗号分隔）
    pub no_proxy: Option<String>,
}

static DETECTED: OnceLock<Option<ProxyConfig>> = OnceLock::new();

/// 探测出站代理（进程内只探测一次）
pub fn detect() -> Option<&'static ProxyConfig> {
    DETECTED.get_or_init(detect_uncached).as_ref()
}

/// 给 reqwest 客户端套上探测到的代理
pub fn apply(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    let Some(p) = detect() else {
        return builder;
    };
    match reqwest::Proxy::all(&p.url) {
        Ok(mut proxy) => {
            if let Some(no_proxy) = p.no_proxy.as_deref().and_then(reqwest::NoProxy::from_string) {
                proxy = proxy.no_proxy(Some(no_proxy));
            }
            builder.proxy(proxy)
        }
        Err(e) => {
            tracing::warn!("代理地址无效（{}）：{e}", p.url);
            builder
        }
    }
}

fn detect_uncached() -> Option<ProxyConfig> {
    if let Some(url) = env_proxy() {
        let url = normalize(&url)?;
        tracing::info!("使用环境变量代理: {url}");
        return Some(ProxyConfig {
            url,
            no_proxy: env_no_proxy(),
        });
    }
    #[cfg(windows)]
    {
        windows_proxy()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

fn env_proxy() -> Option<String> {
    for key in [
        "SPECTATOR_PROXY",
        "HTTPS_PROXY",
        "https_proxy",
        "ALL_PROXY",
        "all_proxy",
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

fn env_no_proxy() -> Option<String> {
    for key in ["NO_PROXY", "no_proxy"] {
        if let Ok(v) = std::env::var(key) {
            let v = v.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// 补全 scheme；不支持的 socks 代理直接忽略
fn normalize(url: &str) -> Option<String> {
    let url = url.trim();
    if url.is_empty() || url.to_ascii_lowercase().starts_with("socks") {
        return None;
    }
    if url.contains("://") {
        return Some(url.to_string());
    }
    Some(format!("http://{url}"))
}

#[cfg(windows)]
fn windows_proxy() -> Option<ProxyConfig> {
    use std::os::windows::process::CommandExt;

    const PATH: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings";
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let out = std::process::Command::new("reg")
        .args(["query", PATH])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);

    let mut enabled = false;
    let mut server = String::new();
    let mut overrides = String::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let Some(name) = parts.next() else { continue };
        // 行形如：ProxyEnable    REG_DWORD    0x1 / ProxyServer    REG_SZ    127.0.0.1:7890
        let value = parts.last().unwrap_or("").trim();
        match name {
            "ProxyEnable" => enabled = value.eq_ignore_ascii_case("0x1"),
            "ProxyServer" => server = value.to_string(),
            "ProxyOverride" => overrides = value.to_string(),
            _ => {}
        }
    }
    if !enabled || server.is_empty() {
        return None;
    }

    let url = normalize(&pick_proxy_server(&server)?)?;
    tracing::info!("检测到 Windows 系统代理，出站请求将经 {url} 转发");
    Some(ProxyConfig {
        url,
        no_proxy: if overrides.trim().is_empty() {
            None
        } else {
            // 系统设置是分号分隔，reqwest 用逗号
            Some(overrides.replace(';', ","))
        },
    })
}

/// 解析系统代理值：`host:port` 或 `http=host:port;https=host:port`
#[cfg(windows)]
fn pick_proxy_server(spec: &str) -> Option<String> {
    if !spec.contains('=') {
        let addr = spec.trim();
        return (!addr.is_empty()).then(|| addr.to_string());
    }
    let mut http = None;
    let mut https = None;
    for part in spec.split(';') {
        let Some((scheme, addr)) = part.split_once('=') else {
            continue;
        };
        let addr = addr.trim();
        if addr.is_empty() {
            continue;
        }
        match scheme.trim().to_ascii_lowercase().as_str() {
            "https" => https = Some(addr.to_string()),
            "http" => http = Some(addr.to_string()),
            _ => {}
        }
    }
    // 这里是 HTTP CONNECT 代理，所以 https= 项同样是 http:// 代理
    http.or(https)
}
