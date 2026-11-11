// 裁判控制台：osu-server-spectator referee hub 的 SignalR 客户端与会话管理
pub mod service;
pub mod signalr;

pub use service::{REF_OAUTH_SCOPES, RefereeService};
