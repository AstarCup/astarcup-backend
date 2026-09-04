// 通用工具：.osu 文件解析、OsuNodeHelper 难度计算、mod 参数构建、
// 敏感词库加载、beatmap URL 解析、本地存储路径处理
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use serde_json::{Value, json};

use crate::config::Config;
use crate::error::AppError;

// ============ .osu 文件解析 ============

/// .osu 完整解析结果
pub struct OsuParsed {
    /// Metadata / Difficulty / General 节的键值对
    pub fields: BTreeMap<String, String>,
    /// 主 BPM（uninherited TimingPoints 出现频率最高的值；无 timing point 为 0）
    pub bpm: f64,
    /// 总长度（秒，取最后一个 HitObject 时间向上取整；无对象为 0）
    pub total_length: i64,
}

/// 解析 .osu：字段 + TimingPoints 计算 BPM + HitObjects 估算时长
pub fn parse_osu_file(content: &str) -> OsuParsed {
    let mut fields = BTreeMap::new();
    let mut current_section = String::new();
    let mut bpms: Vec<f64> = Vec::new();
    let mut last_hit_time: Option<i64> = None;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            current_section = trimmed[1..trimmed.len() - 1].to_string();
            continue;
        }
        match current_section.as_str() {
            "Metadata" | "Difficulty" | "General" => {
                if let Some(idx) = trimmed.find(':') {
                    fields.insert(
                        trimmed[..idx].trim().to_string(),
                        trimmed[idx + 1..].trim().to_string(),
                    );
                }
            }
            // time,beatLength,... — beatLength > 0 即 uninherited（基准 BPM = 60000 / beatLength）
            "TimingPoints" => {
                let mut it = trimmed.split(',');
                let _time = it.next();
                if let Some(bl) = it.next() {
                    if let Ok(v) = bl.trim().parse::<f64>() {
                        if v > 0.0 {
                            bpms.push(60000.0 / v);
                        }
                    }
                }
            }
            // x,y,time,type,hitSound,...
            "HitObjects" => {
                if let Some(t) = trimmed.split(',').nth(2) {
                    if let Ok(v) = t.trim().parse::<i64>() {
                        last_hit_time = Some(last_hit_time.map_or(v, |m: i64| m.max(v)));
                    }
                }
            }
            _ => {}
        }
    }
    // 主 BPM：出现次数最多的值（按 0.01 精度分组）
    let bpm = dominant_bpm(&bpms);
    let total_length = last_hit_time.map(|t| (t + 999) / 1000).unwrap_or(0);
    OsuParsed {
        fields,
        bpm,
        total_length,
    }
}

/// 取列表中出现频率最高的 BPM（四舍五入到 0.01）
fn dominant_bpm(bpms: &[f64]) -> f64 {
    if bpms.is_empty() {
        return 0.0;
    }
    let mut rounded: Vec<f64> = bpms.iter().map(|b| (b * 100.0).round() / 100.0).collect();
    rounded.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut best = rounded[0];
    let mut best_count = 0usize;
    let mut cur = rounded[0];
    let mut cur_count = 0usize;
    for b in rounded {
        if b == cur {
            cur_count += 1;
        } else {
            cur = b;
            cur_count = 1;
        }
        if cur_count > best_count {
            best_count = cur_count;
            best = cur;
        }
    }
    best
}

/// 将 .osu 内容转换为 beatmapInfo JSON
pub fn osu_to_beatmap_info(content: &str, filename: &str, index: usize) -> Value {
    let parsed = parse_osu_file(content);
    let d = &parsed.fields;
    json!({
        "title": d.get("Title").cloned().unwrap_or_default(),
        "titleUnicode": d.get("TitleUnicode").cloned().unwrap_or_else(|| d.get("Title").cloned().unwrap_or_default()),
        "artist": d.get("Artist").cloned().unwrap_or_default(),
        "artistUnicode": d.get("ArtistUnicode").cloned().unwrap_or_else(|| d.get("Artist").cloned().unwrap_or_default()),
        "version": d.get("Version").cloned().unwrap_or_default(),
        "creator": d.get("Creator").cloned().unwrap_or_default(),
        "beatmapId": d.get("BeatmapID").and_then(|v| v.parse::<i64>().ok()).unwrap_or(-1),
        "beatmapsetId": d.get("BeatmapSetID").and_then(|v| v.parse::<i64>().ok()).unwrap_or(-1),
        "cs": d.get("CircleSize").and_then(|v| v.parse::<f64>().ok()).unwrap_or(4.0),
        "ar": d.get("ApproachRate").and_then(|v| v.parse::<f64>().ok()).unwrap_or(9.0),
        "od": d.get("OverallDifficulty").and_then(|v| v.parse::<f64>().ok()).unwrap_or(8.0),
        "hp": d.get("HPDrainRate").and_then(|v| v.parse::<f64>().ok()).unwrap_or(6.0),
        "bpm": parsed.bpm,
        "totalLength": parsed.total_length,
        "tags": d.get("Tags").cloned().unwrap_or_default(),
        "source": d.get("Source").cloned().unwrap_or_default(),
        "osuFilename": filename,
        "index": index,
    })
}

// ============ mod 参数 ============

pub struct ModSettings<'a> {
    pub mods: &'a str,
    pub custom_mod_name: Option<&'a str>,
    pub custom_dt_rate: Option<f64>,
}

/// mods 字符串 → rosu-pp 参数：(legacy bits, clock_rate)。
/// DT 不进 bits（难度上 DT 等价于 1.5x clock_rate），自定义倍率通过 clock_rate 实现。
fn mods_to_bits_and_rate(s: &ModSettings) -> (u32, f64) {
    let mut bits = 0u32;
    let mut rate = 1.0f64;
    if !s.mods.is_empty() && !s.mods.eq_ignore_ascii_case("NM") {
        let m = s.mods.to_uppercase();
        if m == "LZ" {
            // LZ 模式：自定义 lazer mod 不影响难度计算
            if let Some(name) = s.custom_mod_name {
                let cm = name.to_uppercase();
                if cm != "DA"
                    && !["WG", "MR", "RD", "AS", "CL", "SG", "TC", "AC"].contains(&cm.as_str())
                {
                    tracing::info!("未知的 lazer mod: {cm}");
                }
            }
        } else {
            if m.contains("HR") {
                bits |= 16; // HardRock
            }
            if m.contains("HD") {
                bits |= 8; // Hidden
            }
            if m.contains("EZ") {
                bits |= 2; // Easy
            }
            if m.contains("FL") {
                bits |= 1024; // Flashlight
            }
            if m.contains("DT") {
                rate = s.custom_dt_rate.unwrap_or(1.5);
            }
        }
    }
    (bits, rate)
}

// ============ rosu-pp 难度计算 ============

/// 使用 rosu-pp（osu!lazer 难度算法的 Rust 移植）计算谱面难度。
/// 返回与旧 OsuNodeHelper 相同结构的 JSON。
pub async fn calculate_difficulty(
    beatmap_id: Option<i64>,
    settings: &ModSettings<'_>,
    osu_content: Option<&str>,
) -> Result<Value, AppError> {
    // 1. 获取 .osu 内容（优先使用传入内容，否则从 osu! 下载）
    let content = match osu_content {
        Some(c) => c.to_string(),
        None => {
            let bid = beatmap_id.ok_or_else(|| {
                AppError::bad_request("Either beatmapId or osuContent is required")
            })?;
            let resp = reqwest::get(format!("https://osu.ppy.sh/osu/{bid}"))
                .await
                .map_err(|e: reqwest::Error| {
                    AppError::bad_request(format!("获取 beatmap 文件失败: {e}"))
                })?;
            let resp = resp.error_for_status().map_err(|e: reqwest::Error| {
                AppError::bad_request(format!("获取 beatmap 文件失败: {e}"))
            })?;
            let bytes = resp
                .bytes()
                .await
                .map_err(|e| AppError::bad_request(format!("读取 beatmap 文件失败: {e}")))?;
            String::from_utf8_lossy(&bytes).to_string()
        }
    };

    // 2. rosu-pp 解析谱面
    let map = rosu_pp::Beatmap::from_bytes(content.as_bytes())
        .map_err(|e| AppError::bad_request(format!("解析 .osu 文件失败: {e}")))?;

    // 3. 构建难度计算器
    let (bits, rate) = mods_to_bits_and_rate(settings);
    let diff = rosu_pp::Difficulty::new().mods(bits).clock_rate(rate);

    // 4. 难度属性（星级/aim/speed/最大连击等）
    let attrs = diff
        .checked_calculate(&map)
        .map_err(|e| AppError::bad_request(format!("难度计算失败: {e:?}")))?;

    // 5. mod 后的谱面属性（ar/cs/od/hp）
    let battrs = map.attributes().difficulty(&diff).build();

    // 6. 元数据与基础属性
    let parsed = parse_osu_file(&content);
    let d = &parsed.fields;
    let base_bpm = map.bpm();
    let base_len = parsed.total_length as f64;

    // osu!standard 模式的 aim/speed 与物件总数
    let (aim, speed, n_objects) = match &attrs {
        rosu_pp::any::DifficultyAttributes::Osu(a) => (a.aim, a.speed, a.n_objects()),
        _ => (0.0, 0.0, map.hit_objects.len() as u32),
    };

    Ok(json!({
        "star_rating": attrs.stars(),
        "aim_difficulty": aim,
        "speed_difficulty": speed,
        "ar": battrs.ar() as f64,
        "cs": battrs.cs() as f64,
        "od": battrs.od() as f64,
        "hp": battrs.hp() as f64,
        "bpm": base_bpm * rate,
        "length": (base_len / rate).round(),
        "length_seconds": base_len / rate,
        "clock_rate": rate,
        "max_combo": attrs.max_combo(),
        "total_hit_objects": n_objects,
        "ar_base": map.ar as f64,
        "cs_base": map.cs as f64,
        "od_base": map.od as f64,
        "hp_base": map.hp as f64,
        "bpm_base": base_bpm,
        "length_base": base_len,
        "beatmap_id": d.get("BeatmapID").and_then(|v| v.parse::<i64>().ok()).unwrap_or(beatmap_id.unwrap_or(0)),
        "artist": d.get("Artist").cloned().unwrap_or_default(),
        "title": d.get("Title").cloned().unwrap_or_default(),
        "creator": d.get("Creator").cloned().unwrap_or_default(),
    }))
}

/// 构建 modStats JSON（含 DA 自定义覆盖），与原版 calculate-mod-stats 输出一致
#[allow(clippy::too_many_arguments)]
pub fn build_mod_stats(
    full_result: Option<&Value>,
    beatmap_info: &Value,
    settings: &ModSettings,
    custom_da_settings: Option<&Value>,
) -> Value {
    let g = |v: Option<&Value>, k: &str| -> Value {
        v.and_then(|o| o.get(k)).cloned().unwrap_or(Value::Null)
    };
    let num = |v: Value, default: f64| -> f64 { v.as_f64().unwrap_or(default) };
    let da = |k: &str| -> Option<f64> {
        custom_da_settings
            .and_then(|d| d.get(k))
            .and_then(|v| v.as_f64())
    };

    let base = |k: &str, da_k: &str, fallback: f64| -> f64 {
        num(g(full_result, k), 0.0)
            .max(da(da_k).unwrap_or(0.0))
            .max(fallback)
    };
    // 原版逻辑：fullResult?.x || customDASettings?.x || 0
    let pick = |k: &str, da_k: &str| -> Value {
        let fr = g(full_result, k);
        if fr.as_f64().unwrap_or(0.0) != 0.0 {
            return fr;
        }
        if let Some(v) = da(da_k) {
            return json!(v);
        }
        json!(0)
    };

    let star_rating = full_result
        .and_then(|r| r.get("star_rating"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let aim = full_result
        .and_then(|r| r.get("aim_difficulty"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let speed = full_result
        .and_then(|r| r.get("speed_difficulty"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    // bpm/totalLength/元数据：fullResult 有值优先，否则回退 beatmapInfo
    let bpm_val = {
        let b = g(full_result, "bpm");
        if b.as_f64().unwrap_or(0.0) != 0.0 {
            b
        } else {
            beatmap_info.get("bpm").cloned().unwrap_or(json!(0))
        }
    };
    let total_length_val = {
        let l = g(full_result, "length");
        let vf = l.as_f64().unwrap_or(0.0);
        if vf > 0.0 {
            json!(vf as i64)
        } else {
            beatmap_info.get("totalLength").cloned().unwrap_or(json!(0))
        }
    };
    let beatmap_id_val = {
        let b = g(full_result, "beatmap_id");
        if b.as_i64().unwrap_or(0) != 0 {
            b
        } else {
            beatmap_info.get("id").cloned().unwrap_or(Value::Null)
        }
    };
    let artist_val = {
        let a = g(full_result, "artist");
        if !a.as_str().unwrap_or("").is_empty() {
            a
        } else {
            beatmap_info.get("artist").cloned().unwrap_or(json!(""))
        }
    };
    let title_val = {
        let t = g(full_result, "title");
        if !t.as_str().unwrap_or("").is_empty() {
            t
        } else {
            beatmap_info.get("title").cloned().unwrap_or(json!(""))
        }
    };
    let creator_val = {
        let c = g(full_result, "creator");
        if !c.as_str().unwrap_or("").is_empty() {
            c
        } else {
            beatmap_info.get("creator").cloned().unwrap_or(json!(""))
        }
    };

    let mut stats = json!({
        "ar": pick("ar", "ar"),
        "cs": pick("cs", "cs"),
        "od": pick("od", "od"),
        "hp": pick("hp", "hp"),
        "starRating": star_rating,
        "bpm": bpm_val,
        "totalLength": total_length_val,
        "aimDifficulty": aim,
        "speedDifficulty": speed,
        "maxCombo": g(full_result, "max_combo").as_i64().unwrap_or(0),
        "totalHitObjects": g(full_result, "total_hit_objects").as_i64().unwrap_or(0),
        "lengthSeconds": g(full_result, "length_seconds").as_f64().unwrap_or(0.0),
        "clockRate": g(full_result, "clock_rate").as_f64().unwrap_or(1.0),
        "arBase": g(full_result, "ar_base").as_f64().unwrap_or(0.0),
        "csBase": g(full_result, "cs_base").as_f64().unwrap_or(0.0),
        "odBase": g(full_result, "od_base").as_f64().unwrap_or(0.0),
        "hpBase": g(full_result, "hp_base").as_f64().unwrap_or(0.0),
        "bpmBase": g(full_result, "bpm_base").as_f64().unwrap_or(0.0),
        "lengthBase": g(full_result, "length_base").as_f64().unwrap_or(0.0),
        "beatmapId": beatmap_id_val,
        "artist": artist_val,
        "title": title_val,
        "creator": creator_val,
    });

    // DA 模式自定义属性覆盖（mods=LZ && customModName=DA）
    if settings.mods.eq_ignore_ascii_case("LZ")
        && settings
            .custom_mod_name
            .map(|m| m.eq_ignore_ascii_case("DA"))
            .unwrap_or(false)
    {
        if let Some(v) = da("cs") {
            stats["cs"] = json!(v);
        }
        if let Some(v) = da("ar") {
            stats["ar"] = json!(v);
        }
        if let Some(v) = da("od") {
            stats["od"] = json!(v);
        }
        if let Some(v) = da("hp") {
            stats["hp"] = json!(v);
        }
    }
    let _ = base; // 保留兜底语义说明
    stats
}

// ============ 敏感词库 ============

static PROHIBITED_WORDS: OnceLock<Vec<String>> = OnceLock::new();

/// 加载词库目录下全部 .txt（每行一个词），进程内缓存
pub fn prohibited_words(vocab_dir: &str) -> &[String] {
    PROHIBITED_WORDS.get_or_init(|| {
        let mut words = Vec::new();
        if let Ok(entries) = std::fs::read_dir(vocab_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("txt") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        words.extend(
                            content
                                .lines()
                                .map(|w| w.trim().to_string())
                                .filter(|w| !w.is_empty()),
                        );
                    }
                }
            }
        }
        if words.is_empty() {
            tracing::warn!("敏感词库为空或目录不存在: {vocab_dir}");
        }
        words
    })
}

// ============ beatmap URL 解析（与原版 parseBeatmapUrl 一致） ============

#[derive(Debug, Default)]
pub struct ParsedBeatmapUrl {
    pub beatmap_id: Option<i64>,
    pub beatmapset_id: Option<i64>,
}

pub fn parse_beatmap_url(url: &str) -> ParsedBeatmapUrl {
    let trimmed = url.trim();
    if !trimmed.is_empty() && trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return ParsedBeatmapUrl {
            beatmap_id: trimmed.parse().ok(),
            beatmapset_id: None,
        };
    }
    let patterns: [(&str, fn(&str) -> Option<ParsedBeatmapUrl>); 5] = [
        ("osu\\.ppy\\.sh/b/(\\d+)", |url| {
            regex::Regex::new(r"osu\.ppy\.sh/b/(\d+)")
                .ok()
                .and_then(|re| re.captures(url))
                .and_then(|c| c.get(1))
                .and_then(|m| m.as_str().parse().ok())
                .map(|beatmap_id| ParsedBeatmapUrl {
                    beatmap_id: Some(beatmap_id),
                    beatmapset_id: None,
                })
        }),
        ("osu\\.ppy\\.sh/beatmaps/(\\d+)", |url| {
            regex::Regex::new(r"osu\.ppy\.sh/beatmaps/(\d+)")
                .ok()
                .and_then(|re| re.captures(url))
                .and_then(|c| c.get(1))
                .and_then(|m| m.as_str().parse().ok())
                .map(|beatmap_id| ParsedBeatmapUrl {
                    beatmap_id: Some(beatmap_id),
                    beatmapset_id: None,
                })
        }),
        ("beatmapsets/(\\d+)#/…/(\\d+)", |url| {
            regex::Regex::new(r"osu\.ppy\.sh/beatmapsets/(\d+)#[^/]*/(\d+)")
                .ok()
                .and_then(|re| re.captures(url))
                .and_then(|c| {
                    let bid: Option<i64> = c.get(2).and_then(|m| m.as_str().parse().ok());
                    bid.map(|beatmap_id| ParsedBeatmapUrl {
                        beatmap_id: Some(beatmap_id),
                        beatmapset_id: None,
                    })
                })
        }),
        ("osu\\.ppy\\.sh/s/(\\d+)", |url| {
            regex::Regex::new(r"osu\.ppy\.sh/s/(\d+)")
                .ok()
                .and_then(|re| re.captures(url))
                .and_then(|c| c.get(1))
                .and_then(|m| m.as_str().parse().ok())
                .map(|beatmapset_id| ParsedBeatmapUrl {
                    beatmap_id: None,
                    beatmapset_id: Some(beatmapset_id),
                })
        }),
        ("beatmapsets/(\\d+)", |url| {
            regex::Regex::new(r"osu\.ppy\.sh/beatmapsets/(\d+)")
                .ok()
                .and_then(|re| re.captures(url))
                .and_then(|c| c.get(1))
                .and_then(|m| m.as_str().parse().ok())
                .map(|beatmapset_id| ParsedBeatmapUrl {
                    beatmap_id: None,
                    beatmapset_id: Some(beatmapset_id),
                })
        }),
    ];
    for (_, f) in patterns.iter() {
        if let Some(parsed) = f(url) {
            return parsed;
        }
    }
    ParsedBeatmapUrl::default()
}

// ============ 本地存储路径 ============

/// 拼接存储绝对路径，并防止目录穿越
pub fn storage_path(cfg: &Config, relative: &str) -> Result<PathBuf, AppError> {
    let base = Path::new(&cfg.storage_dir);
    let rel = Path::new(relative.trim_start_matches('/'));
    // 拒绝包含 .. 或根路径组件
    for comp in rel.components() {
        match comp {
            Component::Normal(_) => {}
            _ => return Err(AppError::bad_request("非法的存储路径")),
        }
    }
    Ok(base.join(rel))
}

/// 生成自定义图池 .osz 存储路径：/custom/{season}_{category}_{mods}{pos}_{bid}.osz
pub fn custom_osz_path(
    season: &str,
    category: &str,
    selected_mods: &str,
    mod_position: i64,
    beatmap_id: Option<i64>,
) -> String {
    match beatmap_id {
        Some(bid) => format!("/custom/{season}_{category}_{selected_mods}{mod_position}_{bid}.osz"),
        None => format!("/custom/{season}_{category}_{selected_mods}{mod_position}.osz"),
    }
}
