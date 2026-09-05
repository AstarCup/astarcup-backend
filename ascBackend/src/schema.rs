// 数据库 schema（与线上 PostgreSQL 完全一致：表名为 PascalCase，列名与 Prisma 字段名一致）
// 参考 astracup-web/node_modules/.prisma/client/schema.prisma（运行时真值）

use diesel::{allow_tables_to_appear_in_same_query, joinable, table};

table! {
    #[sql_name = "User"]
    users (id) {
        id -> Int4,
        #[sql_name = "osuId"]
        osu_id -> Varchar,
        username -> Varchar,
        #[sql_name = "registeredAt"]
        registered_at -> Timestamp,
        avatar_url -> Nullable<Varchar>,
        pp -> Nullable<Float8>,
        global_rank -> Nullable<Int4>,
        country_rank -> Nullable<Int4>,
        country -> Varchar,
        #[sql_name = "userGroup"]
        user_group -> crate::enums::UserGroupType,
        #[sql_name = "registrationStatus"]
        registration_status -> crate::enums::RegistrationStatusType,
        season -> Nullable<diesel::sql_types::Array<diesel::sql_types::Text>>,
        accuracy -> Nullable<Float8>,
        stamina -> Nullable<Float8>,
        #[sql_name = "firstSight"]
        first_sight -> Nullable<Float8>,
        strategy -> Nullable<Float8>,
        experience -> Nullable<Float8>,
        #[sql_name = "customKey"]
        custom_key -> Nullable<Varchar>,
        #[sql_name = "customValue"]
        custom_value -> Nullable<Float8>,
        cover_custom_url -> Nullable<Varchar>,
        cover_url -> Nullable<Varchar>,
        cover_id -> Nullable<Varchar>,
        #[sql_name = "createdAt"]
        created_at -> Timestamp,
        #[sql_name = "updatedAt"]
        updated_at -> Timestamp,
    }
}

table! {
    #[sql_name = "MatchRoom"]
    match_rooms (id) {
        id -> Int4,
        room_name -> Varchar,
        round_number -> Int4,
        match_date -> Timestamp,
        match_time -> Timestamp,
        match_number -> Int4,
        max_participants -> Int4,
        status -> crate::enums::RoomStatusType,
        description -> Nullable<Varchar>,
        created_by -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    #[sql_name = "MatchSchedule"]
    match_schedules (id) {
        id -> Int4,
        room_id -> Int4,
        #[sql_name = "player1_osuId"]
        player1_osu_id -> Varchar,
        #[sql_name = "player1_username"]
        player1_username -> Varchar,
        #[sql_name = "player2_osuId"]
        player2_osu_id -> Varchar,
        #[sql_name = "player2_username"]
        player2_username -> Varchar,
        red_score -> Int4,
        blue_score -> Int4,
        status -> crate::enums::ScheduleStatusType,
        replay_link -> Nullable<Varchar>,
        match_link -> Nullable<Varchar>,
        stream_link -> Nullable<Varchar>,
        created_by -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    #[sql_name = "PlayerMatchup"]
    player_matchups (id) {
        id -> Int4,
        room_id -> Nullable<Int4>,
        #[sql_name = "player1_osuId"]
        player1_osu_id -> Varchar,
        #[sql_name = "player1_username"]
        player1_username -> Varchar,
        #[sql_name = "player2_osuId"]
        player2_osu_id -> Varchar,
        #[sql_name = "player2_username"]
        player2_username -> Varchar,
        status -> crate::enums::MatchupStatusType,
        created_by -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    #[sql_name = "Message"]
    messages (id) {
        id -> Int4,
        #[sql_name = "sender_osuId"]
        sender_osu_id -> Varchar,
        #[sql_name = "sender_username"]
        sender_username -> Varchar,
        #[sql_name = "receiver_osuId"]
        receiver_osu_id -> Varchar,
        #[sql_name = "receiver_username"]
        receiver_username -> Varchar,
        #[sql_name = "type"]
        message_type -> crate::enums::MessageTypeType,
        title -> Varchar,
        content -> Varchar,
        related_matchup_id -> Nullable<Int4>,
        status -> crate::enums::MessageStatusType,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    #[sql_name = "StaffRoomAssignment"]
    staff_room_assignments (id) {
        id -> Int4,
        room_id -> Int4,
        #[sql_name = "staff_osuId"]
        staff_osu_id -> Varchar,
        #[sql_name = "staff_username"]
        staff_username -> Varchar,
        role -> crate::enums::StaffRoleType,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    #[sql_name = "MatchScore"]
    match_scores (id) {
        id -> Int4,
        schedule_id -> Int4,
        #[sql_name = "player_osuId"]
        player_osu_id -> Varchar,
        #[sql_name = "player_username"]
        player_username -> Varchar,
        score -> Int4,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    #[sql_name = "TournamentSetting"]
    tournament_settings (id) {
        id -> Int4,
        #[sql_name = "setting_key"]
        setting_key -> Varchar,
        #[sql_name = "setting_value"]
        setting_value -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    #[sql_name = "MapSelection"]
    map_selections (id) {
        id -> Int4,
        #[sql_name = "beatmapId"]
        beatmap_id -> Int4,
        #[sql_name = "beatmapsetId"]
        beatmapset_id -> Int4,
        title -> Varchar,
        title_unicode -> Nullable<Varchar>,
        artist -> Varchar,
        artist_unicode -> Nullable<Varchar>,
        version -> Varchar,
        creator -> Varchar,
        #[sql_name = "starRating"]
        star_rating -> Float8,
        bpm -> Float8,
        #[sql_name = "totalLength"]
        total_length -> Int4,
        #[sql_name = "maxCombo"]
        max_combo -> Int4,
        ar -> Float8,
        cs -> Float8,
        od -> Float8,
        hp -> Float8,
        #[sql_name = "selectedMods"]
        selected_mods -> Nullable<Varchar>,
        #[sql_name = "modPosition"]
        mod_position -> Nullable<Int4>,
        comment -> Nullable<Varchar>,
        #[sql_name = "selectedBy"]
        selected_by -> Varchar,
        #[sql_name = "selectedAt"]
        selected_at -> Timestamp,
        season -> Nullable<Varchar>,
        category -> Nullable<Varchar>,
        url -> Nullable<Varchar>,
        #[sql_name = "coverUrl"]
        cover_url -> Nullable<Varchar>,
        approved -> Bool,
        padding -> Bool,
        #[sql_name = "isCustome"]
        is_custome -> Bool,
        #[sql_name = "isOrigin"]
        is_origin -> Bool,
        #[sql_name = "createdAt"]
        created_at -> Timestamp,
        #[sql_name = "updatedAt"]
        updated_at -> Timestamp,
    }
}

table! {
    #[sql_name = "MapComment"]
    map_comments (id) {
        id -> Int4,
        #[sql_name = "mapSelectionId"]
        map_selection_id -> Int4,
        #[sql_name = "userId"]
        user_id -> Varchar,
        username -> Varchar,
        avatar_url -> Varchar,
        comment -> Varchar,
        #[sql_name = "createdAt"]
        created_at -> Timestamp,
        #[sql_name = "updatedAt"]
        updated_at -> Timestamp,
    }
}

joinable!(match_schedules -> match_rooms (room_id));
joinable!(match_scores -> match_schedules (schedule_id));
joinable!(player_matchups -> match_rooms (room_id));
joinable!(staff_room_assignments -> match_rooms (room_id));
joinable!(map_comments -> map_selections (map_selection_id));

allow_tables_to_appear_in_same_query!(
    users,
    match_rooms,
    match_schedules,
    match_scores,
    player_matchups,
    messages,
    staff_room_assignments,
    tournament_settings,
    map_selections,
    map_comments,
);
