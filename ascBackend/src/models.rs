use chrono::NaiveDateTime;
use serde_json::{Value, json};

// diesel 2.3+ 的 derive 宏需显式导入
#[allow(unused_imports, deprecated)]
use diesel::{Identifiable, Queryable, Selectable};

use crate::enums::*;
use crate::schema::*;

// ============ User ============

#[derive(Debug, Queryable, Identifiable, Selectable)]
#[diesel(table_name = users)]
pub struct User {
    pub id: i32,
    pub osu_id: String,
    pub username: String,
    pub registered_at: NaiveDateTime,
    pub avatar_url: Option<String>,
    pub pp: Option<f64>,
    pub global_rank: Option<i32>,
    pub country_rank: Option<i32>,
    pub country: String,
    pub user_group: UserGroup,
    pub registration_status: RegistrationStatus,
    pub season: Option<String>,
    pub accuracy: Option<f64>,
    pub stamina: Option<f64>,
    pub first_sight: Option<f64>,
    pub strategy: Option<f64>,
    pub experience: Option<f64>,
    pub custom_key: Option<String>,
    pub custom_value: Option<f64>,
    pub cover_custom_url: Option<String>,
    pub cover_url: Option<String>,
    pub cover_id: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl User {
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "osuId": self.osu_id,
            "username": self.username,
            "registeredAt": self.registered_at.and_utc().to_rfc3339(),
            "avatarUrl": self.avatar_url,
            "pp": self.pp.unwrap_or(0.0),
            "globalRank": self.global_rank,
            "countryRank": self.country_rank,
            "country": self.country,
            "userGroup": self.user_group.as_str(),
            "registrationStatus": self.registration_status.as_str(),
            "season": self.season,
            "accuracy": self.accuracy,
            "stamina": self.stamina,
            "firstSight": self.first_sight,
            "strategy": self.strategy,
            "experience": self.experience,
            "customKey": self.custom_key,
            "customValue": self.custom_value,
            "cover": self.cover_url.as_ref().map(|url| json!({
                "customUrl": self.cover_custom_url,
                "url": url,
                "id": self.cover_id,
            })),
            "createdAt": self.created_at.and_utc().to_rfc3339(),
            "updatedAt": self.updated_at.and_utc().to_rfc3339(),
        })
    }
}

// ============ MatchRoom ============

#[derive(Debug, Queryable, Identifiable, Selectable)]
#[diesel(table_name = match_rooms)]
pub struct MatchRoom {
    pub id: i32,
    pub room_name: String,
    pub round_number: i32,
    pub match_date: NaiveDateTime,
    pub match_time: NaiveDateTime,
    pub match_number: i32,
    pub max_participants: i32,
    pub status: RoomStatus,
    pub description: Option<String>,
    pub created_by: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl MatchRoom {
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "roomName": self.room_name,
            "roundNumber": self.round_number,
            "matchDate": self.match_date.and_utc().to_rfc3339(),
            "matchTime": self.match_time.and_utc().to_rfc3339(),
            "matchNumber": self.match_number,
            "maxParticipants": self.max_participants,
            "status": self.status.as_str(),
            "description": self.description,
            "createdBy": self.created_by,
            "createdAt": self.created_at.and_utc().to_rfc3339(),
            "updatedAt": self.updated_at.and_utc().to_rfc3339(),
        })
    }
}

// ============ MatchSchedule ============

#[derive(Debug, Queryable, Identifiable, Selectable)]
#[diesel(table_name = match_schedules)]
pub struct MatchSchedule {
    pub id: i32,
    pub room_id: i32,
    pub player1_osu_id: String,
    pub player1_username: String,
    pub player2_osu_id: String,
    pub player2_username: String,
    pub red_score: i32,
    pub blue_score: i32,
    pub status: ScheduleStatus,
    pub replay_link: Option<String>,
    pub match_link: Option<String>,
    pub stream_link: Option<String>,
    pub created_by: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl MatchSchedule {
    pub fn to_json(&self, room: Option<&Value>) -> Value {
        let mut v = json!({
            "id": self.id,
            "roomId": self.room_id,
            "player1OsuId": self.player1_osu_id,
            "player1Username": self.player1_username,
            "player2OsuId": self.player2_osu_id,
            "player2Username": self.player2_username,
            "redScore": self.red_score,
            "blueScore": self.blue_score,
            "status": self.status.as_str(),
            "replayLink": self.replay_link,
            "matchLink": self.match_link,
            "streamLink": self.stream_link,
            "createdBy": self.created_by,
            "createdAt": self.created_at.and_utc().to_rfc3339(),
            "updatedAt": self.updated_at.and_utc().to_rfc3339(),
        });
        if let Some(room) = room {
            v["room"] = room.clone();
        }
        v
    }
}

// ============ PlayerMatchup ============

#[derive(Debug, Queryable, Identifiable, Selectable)]
#[diesel(table_name = player_matchups)]
pub struct PlayerMatchup {
    pub id: i32,
    pub room_id: Option<i32>,
    pub player1_osu_id: String,
    pub player1_username: String,
    pub player2_osu_id: String,
    pub player2_username: String,
    pub status: MatchupStatus,
    pub created_by: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl PlayerMatchup {
    pub fn to_json(&self, room: Option<&Value>) -> Value {
        let mut v = json!({
            "id": self.id,
            "roomId": self.room_id,
            "player1OsuId": self.player1_osu_id,
            "player1Username": self.player1_username,
            "player2OsuId": self.player2_osu_id,
            "player2Username": self.player2_username,
            "status": self.status.as_str(),
            "createdBy": self.created_by,
            "createdAt": self.created_at.and_utc().to_rfc3339(),
            "updatedAt": self.updated_at.and_utc().to_rfc3339(),
        });
        if let Some(room) = room {
            v["room"] = room.clone();
        }
        v
    }
}

// ============ Message ============

#[derive(Debug, Queryable, Identifiable, Selectable)]
#[diesel(table_name = messages)]
pub struct Message {
    pub id: i32,
    pub sender_osu_id: String,
    pub sender_username: String,
    pub receiver_osu_id: String,
    pub receiver_username: String,
    pub message_type: MessageType,
    pub title: String,
    pub content: String,
    pub related_matchup_id: Option<i32>,
    pub status: MessageStatus,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl Message {
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "senderOsuId": self.sender_osu_id,
            "senderUsername": self.sender_username,
            "receiverOsuId": self.receiver_osu_id,
            "receiverUsername": self.receiver_username,
            "type": self.message_type.as_str(),
            "title": self.title,
            "content": self.content,
            "relatedMatchupId": self.related_matchup_id,
            "status": self.status.as_str(),
            "createdAt": self.created_at.and_utc().to_rfc3339(),
            "updatedAt": self.updated_at.and_utc().to_rfc3339(),
        })
    }
}

// ============ StaffRoomAssignment ============

#[derive(Debug, Queryable, Identifiable, Selectable)]
#[diesel(table_name = staff_room_assignments)]
pub struct StaffRoomAssignment {
    pub id: i32,
    pub room_id: i32,
    pub staff_osu_id: String,
    pub staff_username: String,
    pub role: StaffRole,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl StaffRoomAssignment {
    pub fn to_json(&self, room: Option<&Value>) -> Value {
        let mut v = json!({
            "id": self.id,
            "roomId": self.room_id,
            "staffOsuId": self.staff_osu_id,
            "staffUsername": self.staff_username,
            "role": self.role.as_str(),
            "createdAt": self.created_at.and_utc().to_rfc3339(),
            "updatedAt": self.updated_at.and_utc().to_rfc3339(),
        });
        if let Some(room) = room {
            v["room"] = room.clone();
        }
        v
    }
}

// ============ MatchScore ============

#[derive(Debug, Queryable, Identifiable, Selectable)]
#[diesel(table_name = match_scores)]
pub struct MatchScore {
    pub id: i32,
    pub schedule_id: i32,
    pub player_osu_id: String,
    pub player_username: String,
    pub score: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl MatchScore {
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "scheduleId": self.schedule_id,
            "playerOsuId": self.player_osu_id,
            "playerUsername": self.player_username,
            "score": self.score,
            "createdAt": self.created_at.and_utc().to_rfc3339(),
            "updatedAt": self.updated_at.and_utc().to_rfc3339(),
        })
    }
}

// ============ TournamentSetting ============

#[derive(Debug, Queryable, Identifiable, Selectable)]
#[diesel(table_name = tournament_settings)]
pub struct TournamentSetting {
    pub id: i32,
    pub setting_key: String,
    pub setting_value: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

// ============ MapSelection ============

#[derive(Debug, Queryable, Identifiable, Selectable)]
#[diesel(table_name = map_selections)]
pub struct MapSelection {
    pub id: i32,
    pub beatmap_id: i32,
    pub beatmapset_id: i32,
    pub title: String,
    pub title_unicode: Option<String>,
    pub artist: String,
    pub artist_unicode: Option<String>,
    pub version: String,
    pub creator: String,
    pub star_rating: f64,
    pub bpm: f64,
    pub total_length: i32,
    pub max_combo: i32,
    pub ar: f64,
    pub cs: f64,
    pub od: f64,
    pub hp: f64,
    pub selected_mods: Option<String>,
    pub mod_position: Option<i32>,
    pub comment: Option<String>,
    pub selected_by: String,
    pub selected_at: NaiveDateTime,
    pub season: Option<String>,
    pub category: Option<String>,
    pub url: Option<String>,
    pub cover_url: Option<String>,
    pub approved: bool,
    pub padding: bool,
    pub is_custome: bool,
    pub is_origin: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl MapSelection {
    /// 输出与前端兼容：selectedMods 若为 JSON 编码则拆出 customDTRate / customModName
    pub fn to_json(&self, selected_by_user: Option<(&str, &str)>) -> Value {
        let mods_raw = self.selected_mods.clone().unwrap_or_default();
        let (mods, custom_dt_rate, custom_mod_name) = match serde_json::from_str::<Value>(&mods_raw)
        {
            Ok(obj) if obj.is_object() => (
                obj.get("mod")
                    .and_then(|m| m.as_str())
                    .unwrap_or(&mods_raw)
                    .to_string(),
                obj.get("customDTRate").cloned().unwrap_or(Value::Null),
                obj.get("customModName").cloned().unwrap_or(Value::Null),
            ),
            _ => (mods_raw.clone(), Value::Null, Value::Null),
        };

        let mut v = json!({
            "id": self.id,
            "beatmapId": self.beatmap_id,
            "beatmapsetId": self.beatmapset_id,
            "title": self.title,
            "title_unicode": self.title_unicode.clone().unwrap_or_else(|| self.title.clone()),
            "artist": self.artist,
            "artist_unicode": self.artist_unicode.clone().unwrap_or_else(|| self.artist.clone()),
            "version": self.version,
            "creator": self.creator,
            "starRating": self.star_rating,
            "bpm": self.bpm,
            "totalLength": self.total_length,
            "maxCombo": self.max_combo,
            "ar": self.ar,
            "cs": self.cs,
            "od": self.od,
            "hp": self.hp,
            "selectedMods": mods,
            "modPosition": self.mod_position.unwrap_or(1),
            "comment": self.comment.clone().unwrap_or_default(),
            "selectedBy": self.selected_by,
            "selectedAt": self.selected_at.and_utc().to_rfc3339(),
            "season": self.season,
            "category": self.category,
            "url": self.url,
            "coverUrl": self.cover_url,
            "approved": self.approved,
            "padding": self.padding,
            "isCustome": self.is_custome,
            "isOrigin": self.is_origin,
            "createdAt": self.created_at.and_utc().to_rfc3339(),
            "updatedAt": self.updated_at.and_utc().to_rfc3339(),
        });
        if custom_dt_rate != Value::Null {
            v["customDTRate"] = custom_dt_rate;
        }
        if custom_mod_name != Value::Null {
            v["customModName"] = custom_mod_name;
        }
        if let Some((username, avatar)) = selected_by_user {
            v["selectedByUsername"] = json!(username);
            v["selectedByAvatar"] = json!(avatar);
        }
        v
    }

    /// selectedMods 入库编码：带自定义项时存 JSON
    pub fn encode_mods(
        mods: &str,
        custom_dt_rate: Option<f64>,
        custom_mod_name: Option<&str>,
    ) -> String {
        if custom_dt_rate.is_none() && custom_mod_name.is_none() {
            return mods.to_string();
        }
        let mut obj = serde_json::Map::new();
        obj.insert("mod".into(), json!(mods));
        if let Some(rate) = custom_dt_rate {
            obj.insert("customDTRate".into(), json!(rate));
        }
        if let Some(name) = custom_mod_name {
            obj.insert("customModName".into(), json!(name));
        }
        Value::Object(obj).to_string()
    }

    /// 从已存的 selectedMods 提取基础 mod 名
    pub fn base_mod(mods_stored: &str) -> String {
        match serde_json::from_str::<Value>(mods_stored) {
            Ok(obj) if obj.is_object() => obj
                .get("mod")
                .and_then(|m| m.as_str())
                .unwrap_or(mods_stored)
                .to_string(),
            _ => mods_stored.to_string(),
        }
    }
}

// ============ MapComment ============

#[derive(Debug, Queryable, Identifiable, Selectable)]
#[diesel(table_name = map_comments)]
pub struct MapComment {
    pub id: i32,
    pub map_selection_id: i32,
    pub user_id: String,
    pub username: String,
    pub avatar_url: String,
    pub comment: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl MapComment {
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "mapSelectionId": self.map_selection_id,
            "userId": self.user_id,
            "username": self.username,
            "avatarUrl": self.avatar_url,
            "comment": self.comment,
            "createdAt": self.created_at.and_utc().to_rfc3339(),
            "updatedAt": self.updated_at.and_utc().to_rfc3339(),
        })
    }
}
