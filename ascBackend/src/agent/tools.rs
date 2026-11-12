// 裁判 agent 的工具定义与执行器：图池、比赛手册、对局信息、比分与比赛控制
use diesel::prelude::*;
use serde_json::{Value, json};

use crate::db::{DbPool, run_db};
use crate::models::MapSelection;
use crate::referee::service::RefereeService;
use crate::schema::{map_selections, match_schedules, season_rules, tournament_settings};

/// 会话内状态：图池上下文与 ban/pick 记录
#[derive(Default)]
pub struct SessionState {
    pub pool_season: Option<String>,
    pub pool_category: Option<String>,
    pub bans: Vec<String>,
    pub picks: Vec<String>,
    /// BP 阶段窗口：开启时玩家才可通过聊天 ban/pick
    pub bp_active: bool,
}

pub struct ToolCtx<'a> {
    pub osu_id: &'a str,
    pub room_id: i64,
    pub chat_channel_id: Option<i64>,
    pub current_playlist_item_id: Option<i64>,
    /// 是否允许改变比赛状态的工具（玩家消息触发的轮次为 false）
    pub allow_mutating: bool,
    pub session: &'a mut SessionState,
    pub svc: &'a RefereeService,
    pub pool: &'a DbPool,
    pub http: &'a reqwest::Client,
}

/// 阶段短码 → 图池 category 存储值
fn normalize_stage(stage: &str) -> String {
    let s = stage.trim().to_lowercase();
    match s.as_str() {
        "qua" | "qualification" => "qualification".into(),
        "ro16" => "ro16".into(),
        "ro32" => "ro32".into(),
        "qf" | "quarterfinals" => "quarterfinals".into(),
        "sf" | "semifinals" => "semifinals".into(),
        "f" | "finals" => "finals".into(),
        "gf" | "grandfinals" => "grandfinals".into(),
        _ => s,
    }
}

/// 工具定义（OpenAI function calling 格式）
pub fn tool_defs() -> Vec<Value> {
    let defs: &[(&str, &str, Value)] = &[
        (
            "announce",
            "向房间聊天发送裁判播报（玩家在 osu! 内可见），同时显示在裁判控制台。所有对玩家的说明、比分播报、流程提示都用它。",
            json!({"type":"object","properties":{"text":{"type":"string","description":"播报内容"}},"required":["text"]}),
        ),
        (
            "pick_map",
            "按图位（如 NM1、HD2、DT1）从当前图池选一张图加入房间图池队列（自动带对应 mod：HD 图附带 Deflate(start_size=1)，其他图附带 Approach Different(style=0)）。",
            json!({"type":"object","properties":{"slot":{"type":"string","description":"图位，如 NM1"}},"required":["slot"]}),
        ),
        (
            "ban_map",
            "在 BP 阶段 ban 掉图池中的一张图（记录在会话状态里）。",
            json!({"type":"object","properties":{"slot":{"type":"string","description":"图位，如 NM1"}},"required":["slot"]}),
        ),
        (
            "add_beatmap",
            "加入任意谱面到房间图池队列（热手图、自由选图等）。",
            json!({"type":"object","properties":{
                "beatmap_id":{"type":"integer","description":"osu! beatmap ID"},
                "freestyle":{"type":"boolean","description":"是否 freestyle（默认 true 用于热手）"}}
            ,"required":["beatmap_id"]}),
        ),
        (
            "roll",
            "发起一次 roll（双方玩家在聊天里会看到结果）。",
            json!({"type":"object","properties":{"max":{"type":"integer","description":"roll 上限，默认 100"}}}),
        ),
        (
            "start_match",
            "开始比赛（可带官方倒计时秒数，0 = 立即）。",
            json!({"type":"object","properties":{"countdown_seconds":{"type":"integer","description":"倒计时秒数，默认 0"}}}),
        ),
        (
            "stop_countdown",
            "停止比赛倒计时。",
            json!({"type":"object","properties":{}}),
        ),
        (
            "abort_match",
            "中止当前比赛。",
            json!({"type":"object","properties":{}}),
        ),
        (
            "start_bp",
            "进入 ban/pick 阶段（清空并重置 ban/pick 记录；开启后玩家可以在聊天里自行 ban/pick）。在 roll 定序完成后调用。",
            json!({"type":"object","properties":{}}),
        ),
        (
            "end_bp",
            "结束 ban/pick 阶段（关闭玩家自行 ban/pick 的权限）。比赛分出胜负、BP 全部完成后调用。",
            json!({"type":"object","properties":{}}),
        ),
        (
            "set_pool",
            "设置图池上下文（赛季与阶段）。阶段用短码：qua/ro16/qf/sf/f/gf。",
            json!({"type":"object","properties":{
                "season":{"type":"string","description":"赛季，如 s1"},
                "stage":{"type":"string","description":"阶段短码，如 gf"}}
            ,"required":["season","stage"]}),
        ),
        (
            "list_pool",
            "列出当前图池全部图位及状态（可选 / 已 ban / 已 pick）。",
            json!({"type":"object","properties":{}}),
        ),
        (
            "get_scores",
            "获取当前小节比分（osu! API）。",
            json!({"type":"object","properties":{"playlist_item_id":{"type":"integer","description":"可选；默认用当前图"}}}),
        ),
        (
            "get_match_info",
            "获取对局信息（当前赛季的赛程、选手、房间安排）。",
            json!({"type":"object","properties":{}}),
        ),
        (
            "get_rules",
            "获取比赛手册 / 规则（赛季规则文档）。",
            json!({"type":"object","properties":{"season":{"type":"string","description":"可选，默认当前赛季"}}}),
        ),
        (
            "get_settings",
            "获取比赛设置（当前赛季、当前阶段等站点设置）。",
            json!({"type":"object","properties":{}}),
        ),
    ];
    defs.iter()
        .map(|(name, desc, params)| {
            json!({
                "type": "function",
                "function": {
                    "name": name,
                    "description": desc,
                    "parameters": params,
                }
            })
        })
        .collect()
}

// ============ 数据查询 ============

async fn load_pool(ctx: &ToolCtx<'_>) -> Result<Vec<Value>, String> {
    let season = ctx
        .session
        .pool_season
        .clone()
        .ok_or_else(|| "图池未设置：请先调用 set_pool（赛季 + 阶段）".to_string())?;
    let category = ctx
        .session
        .pool_category
        .clone()
        .ok_or_else(|| "图池未设置：请先调用 set_pool（赛季 + 阶段）".to_string())?;
    let pool = ctx.pool.clone();
    let rows = run_db(&pool, move |conn| {
        map_selections::table
            .filter(map_selections::season.eq(season))
            .filter(map_selections::category.eq(category))
            .filter(map_selections::approved.eq(true))
            .order((map_selections::selected_mods, map_selections::mod_position))
            .load::<MapSelection>(conn)
            .map_err(crate::error::AppError::from)
    })
    .await
    .map_err(|e| e.message)?;
    Ok(rows.iter().map(|m| m.to_json(None)).collect())
}

fn slot_label(map: &Value) -> String {
    let mods = map["selectedMods"].as_str().unwrap_or("NM").to_uppercase();
    let pos = map["modPosition"].as_i64().unwrap_or(0);
    format!("{mods}{pos}")
}

/// 从图池条目构造 required/allowed mods（含赛事 AD/DF 规则）
fn build_mods(map: &Value) -> (Vec<Value>, Vec<Value>) {
    let mods_raw = map["selectedMods"].as_str().unwrap_or("NM").to_uppercase();
    let dt_rate = map["customDTRate"].as_f64();
    let mut required: Vec<Value> = Vec::new();
    let mut allowed: Vec<Value> = Vec::new();
    for token in mods_raw
        .split('+')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        match token {
            "NM" | "TB" => {}
            "FM" => {
                for m in [
                    "EZ", "NF", "HT", "DC", "HD", "HR", "SD", "PF", "DT", "NC", "FL", "TD", "SO",
                    "MR", "RD",
                ] {
                    allowed.push(json!({"acronym": m}));
                }
            }
            "DT" => match dt_rate {
                Some(rate) => {
                    required.push(json!({"acronym": "DT", "settings": {"speed_change": rate}}))
                }
                None => required.push(json!({"acronym": "DT"})),
            },
            other => required.push(json!({"acronym": other})),
        }
    }
    // 赛事规则：HD 图附带 Deflate(start_size=1)，其他图附带 Approach Different(style=0)
    if required.iter().any(|m| m["acronym"] == "HD") {
        required.push(json!({"acronym": "DF", "settings": {"start_size": 1}}));
    } else {
        required.push(json!({"acronym": "AD", "settings": {"style": 0}}));
    }
    (required, allowed)
}

// ============ 工具执行 ============

/// 返回 (工具结果, announce 播报记录)
pub async fn execute(
    ctx: &mut ToolCtx<'_>,
    name: &str,
    args: &Value,
) -> Result<(Value, Option<String>), String> {
    // 权限闸（不依赖模型自觉）：
    // - 仅裁判/系统事件可触发：其余改变比赛状态的工具；
    // - 玩家消息只能在 BP 阶段动 ban/pick，其他时间连 ban/pick 也不许。
    const REF_ONLY: &[&str] = &[
        "add_beatmap",
        "roll",
        "start_match",
        "stop_countdown",
        "abort_match",
        "set_pool",
        "start_bp",
        "end_bp",
    ];
    const PLAYER_BP_TOOLS: &[&str] = &["pick_map", "ban_map"];
    if !ctx.allow_mutating {
        if REF_ONLY.contains(&name) {
            return Err("权限不足：该操作仅限裁判触发，不能因玩家消息执行".into());
        }
        if PLAYER_BP_TOOLS.contains(&name) && !ctx.session.bp_active {
            return Err("当前不在 ban/pick 阶段，玩家无法执行该操作".into());
        }
    }
    let room_id = json!(ctx.room_id);
    match name {
        "announce" => {
            let text = args["text"].as_str().unwrap_or("").trim().to_string();
            if text.is_empty() {
                return Err("announce 的 text 不能为空".into());
            }
            let Some(channel) = ctx.chat_channel_id else {
                return Err("未知聊天频道，无法播报（请确认已在房间内）".into());
            };
            ctx.svc
                .chat_send(ctx.osu_id, channel, &text)
                .await
                .map_err(|e| e.message)?;
            Ok((json!(format!("已播报：{text}")), Some(text)))
        }

        "pick_map" => {
            let slot = args["slot"].as_str().unwrap_or("").trim().to_uppercase();
            if slot.is_empty() {
                return Err("缺少 slot（如 NM1）".into());
            }
            let maps = load_pool(ctx).await?;
            let map = maps
                .iter()
                .find(|m| slot_label(m) == slot)
                .ok_or_else(|| format!("图池里没有 {slot}，用 list_pool 查看图位"))?;
            if ctx.session.bans.contains(&slot) {
                return Err(format!("{slot} 已被 ban，不能选"));
            }
            if ctx.session.picks.contains(&slot) {
                return Err(format!("{slot} 已经选过"));
            }
            let (required, allowed) = build_mods(map);
            let beatmap_id = map["beatmapId"].as_i64().unwrap_or(0);
            let result = ctx
                .svc
                .invoke(
                    ctx.osu_id,
                    "AddPlaylistItem",
                    vec![
                        room_id.clone(),
                        json!({
                            "beatmap_id": beatmap_id,
                            "ruleset_id": 0,
                            "required_mods": required,
                            "allowed_mods": allowed,
                            "freestyle": false,
                        }),
                    ],
                )
                .await
                .map_err(|e| e.message)?;
            ctx.session.picks.push(slot.clone());
            let title = format!(
                "{} - {} [{}]",
                map["artist"].as_str().unwrap_or("?"),
                map["title"].as_str().unwrap_or("?"),
                map["version"].as_str().unwrap_or("?"),
            );
            let _ = result;
            Ok((json!(format!("已选 {slot}：{title}")), None))
        }

        "ban_map" => {
            let slot = args["slot"].as_str().unwrap_or("").trim().to_uppercase();
            if slot.is_empty() {
                return Err("缺少 slot（如 NM1）".into());
            }
            let maps = load_pool(ctx).await?;
            if !maps.iter().any(|m| slot_label(m) == slot) {
                return Err(format!("图池里没有 {slot}，用 list_pool 查看图位"));
            }
            if ctx.session.bans.contains(&slot) || ctx.session.picks.contains(&slot) {
                return Err(format!("{slot} 已被 ban 或已选过"));
            }
            ctx.session.bans.push(slot.clone());
            Ok((json!(format!("{slot} 已 ban")), None))
        }

        "add_beatmap" => {
            let beatmap_id = args["beatmap_id"].as_i64().ok_or("缺少 beatmap_id")?;
            let freestyle = args["freestyle"].as_bool().unwrap_or(true);
            ctx.svc
                .invoke(
                    ctx.osu_id,
                    "AddPlaylistItem",
                    vec![
                        room_id.clone(),
                        json!({
                            "beatmap_id": beatmap_id,
                            "ruleset_id": 0,
                            "required_mods": [],
                            "allowed_mods": [],
                            "freestyle": freestyle,
                        }),
                    ],
                )
                .await
                .map_err(|e| e.message)?;
            Ok((json!(format!("谱面 {beatmap_id} 已加入图池队列")), None))
        }

        "roll" => {
            let max = args["max"].as_i64().unwrap_or(100);
            ctx.svc
                .invoke(
                    ctx.osu_id,
                    "Roll",
                    vec![room_id.clone(), json!({"max": max})],
                )
                .await
                .map_err(|e| e.message)?;
            Ok((json!(format!("已发起 roll（1-{max}）")), None))
        }

        "start_match" => {
            let countdown = args["countdown_seconds"].as_i64().unwrap_or(0);
            ctx.svc
                .invoke(
                    ctx.osu_id,
                    "StartMatch",
                    vec![room_id.clone(), json!({"countdown": countdown})],
                )
                .await
                .map_err(|e| e.message)?;
            Ok((json!(format!("比赛已开始（倒计时 {countdown} 秒）")), None))
        }

        "stop_countdown" => {
            ctx.svc
                .invoke(ctx.osu_id, "StopMatchCountdown", vec![room_id.clone()])
                .await
                .map_err(|e| e.message)?;
            Ok((json!("倒计时已停止"), None))
        }

        "abort_match" => {
            ctx.svc
                .invoke(ctx.osu_id, "AbortMatch", vec![room_id.clone()])
                .await
                .map_err(|e| e.message)?;
            Ok((json!("比赛已中止"), None))
        }

        "start_bp" => {
            ctx.session.bp_active = true;
            ctx.session.bans.clear();
            ctx.session.picks.clear();
            Ok((
                json!("已进入 ban/pick 阶段：玩家现在可以自行 ban/pick"),
                None,
            ))
        }

        "end_bp" => {
            ctx.session.bp_active = false;
            Ok((json!("ban/pick 阶段已结束"), None))
        }

        "set_pool" => {
            let season = args["season"].as_str().unwrap_or("").trim().to_lowercase();
            let stage = args["stage"].as_str().unwrap_or("").trim().to_lowercase();
            if season.is_empty() || stage.is_empty() {
                return Err("set_pool 需要 season 与 stage".into());
            }
            ctx.session.pool_season = Some(season.clone());
            ctx.session.pool_category = Some(normalize_stage(&stage));
            let maps = load_pool(ctx).await?;
            Ok((
                json!(format!(
                    "图池已设置为 {season} / {}（{} 张图）",
                    ctx.session.pool_category.clone().unwrap_or_default(),
                    maps.len()
                )),
                None,
            ))
        }

        "list_pool" => {
            let maps = load_pool(ctx).await?;
            if maps.is_empty() {
                return Ok((json!("图池为空"), None));
            }
            let lines: Vec<String> = maps
                .iter()
                .map(|m| {
                    let slot = slot_label(m);
                    let status = if ctx.session.bans.contains(&slot) {
                        "BANNED"
                    } else if ctx.session.picks.contains(&slot) {
                        "PICKED"
                    } else {
                        "可选"
                    };
                    format!(
                        "{slot} {} - {} [{}] ({status})",
                        m["artist"].as_str().unwrap_or("?"),
                        m["title"].as_str().unwrap_or("?"),
                        m["version"].as_str().unwrap_or("?"),
                    )
                })
                .collect();
            Ok((json!(lines.join("\n")), None))
        }

        "get_scores" => {
            let playlist_id = args["playlist_item_id"]
                .as_i64()
                .or(ctx.current_playlist_item_id)
                .ok_or("未知当前图（playlist_item_id），请指定")?;
            let token = ctx
                .svc
                .ensure_token(ctx.osu_id)
                .await
                .map_err(|e| e.message)?;
            let url = format!(
                "https://osu.ppy.sh/api/v2/rooms/{}/playlist/{}/scores",
                ctx.room_id, playlist_id
            );
            let resp = ctx
                .http
                .get(url)
                .bearer_auth(token)
                .send()
                .await
                .map_err(|e| format!("获取比分失败: {e}"))?;
            let body: Value = resp
                .json()
                .await
                .map_err(|e| format!("比分解析失败: {e}"))?;
            let scores = body["scores"].as_array().cloned().unwrap_or_default();
            let lines: Vec<String> = scores
                .iter()
                .map(|s| {
                    format!(
                        "{} {}分 acc {}%",
                        s["user"]["username"].as_str().unwrap_or("?"),
                        s["total_score"].as_i64().unwrap_or(0),
                        (s["accuracy"].as_f64().unwrap_or(0.0) * 100.0).round() / 100.0,
                    )
                })
                .collect();
            Ok((json!(lines.join("\n")), None))
        }

        "get_match_info" => {
            let season = ctx.session.pool_season.clone();
            let pool = ctx.pool.clone();
            let rows = run_db(&pool, move |conn| {
                let mut q = match_schedules::table.into_boxed();
                if let Some(s) = season {
                    q = q.filter(match_schedules::season.eq(s));
                }
                q.order(match_schedules::created_at.desc())
                    .limit(20)
                    .load::<crate::models::MatchSchedule>(conn)
                    .map_err(crate::error::AppError::from)
            })
            .await
            .map_err(|e| e.message)?;
            let list: Vec<Value> = rows.iter().map(|s| s.to_json(None)).collect();
            Ok((json!(list), None))
        }

        "get_rules" => {
            let season = args["season"]
                .as_str()
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .or_else(|| ctx.session.pool_season.clone())
                .ok_or("未指定赛季（set_pool 或传 season 参数）")?;
            let pool = ctx.pool.clone();
            let row = run_db(&pool, move |conn| {
                season_rules::table
                    .filter(season_rules::season.eq(season))
                    .select(season_rules::content)
                    .first::<String>(conn)
                    .optional()
                    .map_err(crate::error::AppError::from)
            })
            .await
            .map_err(|e| e.message)?;
            let content = row.unwrap_or_else(|| "（该赛季没有规则文档）".into());
            // 限制上下文长度（涵盖完整赛制段落，避免截掉图池表等关键行）
            let trimmed: String = content.chars().take(8000).collect();
            Ok((json!(trimmed), None))
        }

        "get_settings" => {
            let pool = ctx.pool.clone();
            let rows = run_db(&pool, move |conn| {
                tournament_settings::table
                    .load::<crate::models::TournamentSetting>(conn)
                    .map_err(crate::error::AppError::from)
            })
            .await
            .map_err(|e| e.message)?;
            let map: serde_json::Map<String, Value> = rows
                .iter()
                .map(|r| (r.setting_key.clone(), json!(r.setting_value)))
                .collect();
            Ok((json!(Value::Object(map)), None))
        }

        other => Err(format!("未知工具：{other}")),
    }
}
