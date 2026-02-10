use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 创建枚举类型
        manager
            .create_type(
                Type::create()
                    .as_enum(BeatmapSelectMod::Table)
                    .values([
                        BeatmapSelectMod::Nm,
                        BeatmapSelectMod::Hd,
                        BeatmapSelectMod::Hr,
                        BeatmapSelectMod::Dt,
                        BeatmapSelectMod::Lz,
                        BeatmapSelectMod::Tb,
                    ])
                    .to_owned(),
            )
            .await?;

        manager
            .create_type(
                Type::create()
                    .as_enum(MultiplayerRoomType::Table)
                    .values([
                        MultiplayerRoomType::Solo,
                        MultiplayerRoomType::TeamVs,
                    ])
                    .to_owned(),
            )
            .await?;

        manager
            .create_type(
                Type::create()
                    .as_enum(TeamColor::Table)
                    .values([
                        TeamColor::BlueTeam,
                        TeamColor::RedTeam,
                    ])
                    .to_owned(),
            )
            .await?;

        manager
            .create_type(
                Type::create()
                    .as_enum(TeamState::Table)
                    .values([
                        TeamState::Active,
                        TeamState::Unapproved,
                        TeamState::Approved,
                        TeamState::Banned,
                    ])
                    .to_owned(),
            )
            .await?;

        manager
            .create_type(
                Type::create()
                    .as_enum(UserState::Table)
                    .values([
                        UserState::Active,
                        UserState::Registered,
                        UserState::Abandoned,
                        UserState::Banned,
                    ])
                    .to_owned(),
            )
            .await?;

        manager
            .create_type(
                Type::create()
                    .as_enum(UserGroupType::Table)
                    .values([
                        UserGroupType::Host,
                        UserGroupType::Admin,
                        UserGroupType::Pooler,
                        UserGroupType::Streamer,
                        UserGroupType::Tester,
                        UserGroupType::Gfx,
                        UserGroupType::Sheeter,
                        UserGroupType::Commentator,
                        UserGroupType::Mapper,
                        UserGroupType::Referee,
                        UserGroupType::Player,
                    ])
                    .to_owned(),
            )
            .await?;

        manager
            .create_type(
                Type::create()
                    .as_enum(Season::Table)
                    .values([
                        Season::S1,
                        Season::S2,
                    ])
                    .to_owned(),
            )
            .await?;

        manager
            .create_type(
                Type::create()
                    .as_enum(Category::Table)
                    .values([
                        Category::Qua,
                        Category::Ro16,
                        Category::Qf,
                        Category::Sf,
                        Category::F,
                        Category::Gf,
                    ])
                    .to_owned(),
            )
            .await?;

        // 创建用户表
        manager
            .create_table(
                Table::create()
                    .table(User::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(User::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(User::UserState).custom(UserState::Table).not_null().default("ACTIVE"))
                    .col(ColumnDef::new(User::Osuid).integer().not_null().unique_key())
                    .col(ColumnDef::new(User::Username).string().not_null().unique_key())
                    .col(ColumnDef::new(User::AvatarUrl).string().null())
                    .col(ColumnDef::new(User::CoverUrl).string().null())
                    .col(ColumnDef::new(User::CountryCode).string().not_null())
                    .col(ColumnDef::new(User::Pp).double().not_null().default(0.0))
                    .col(ColumnDef::new(User::GlobalRank).integer().not_null().default(0))
                    .col(ColumnDef::new(User::CountryRank).integer().not_null().default(0))
                    .col(ColumnDef::new(User::Approved).boolean().not_null().default(false))
                    .col(ColumnDef::new(User::Seed).integer().not_null().default(0))
                    .col(ColumnDef::new(User::Seasonal).custom(Season::Table).not_null().default("S1"))
                    .col(ColumnDef::new(User::CreatedAt).timestamp().not_null().default(Expr::current_timestamp()))
                    .col(ColumnDef::new(User::UpdatedAt).timestamp().not_null().default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await?;

        // 创建队伍表
        manager
            .create_table(
                Table::create()
                    .table(Team::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Team::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Team::Name).string().not_null())
                    .col(ColumnDef::new(Team::BannerUrl).string().not_null())
                    .col(ColumnDef::new(Team::TeamColor).custom(TeamColor::Table).not_null())
                    .col(ColumnDef::new(Team::TeamState).custom(TeamState::Table).not_null().default("ACTIVE"))
                    .col(ColumnDef::new(Team::CreatedAt).timestamp().not_null().default(Expr::current_timestamp()))
                    .col(ColumnDef::new(Team::UpdatedAt).timestamp().not_null().default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await?;

        // 创建比赛配置表
        manager
            .create_table(
                Table::create()
                    .table(TournamentConfig::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TournamentConfig::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(TournamentConfig::TournamentName).string().not_null().default("AstarCup"))
                    .col(ColumnDef::new(TournamentConfig::MaxPpForRegistration).double().not_null().default(0.0))
                    .col(ColumnDef::new(TournamentConfig::MinPpForRegistration).double().not_null().default(0.0))
                    .col(ColumnDef::new(TournamentConfig::CurrentSeasonal).custom(Season::Table).not_null().default("S1"))
                    .col(ColumnDef::new(TournamentConfig::CurrentCategory).custom(Category::Table).not_null().default("QUA"))
                    .col(ColumnDef::new(TournamentConfig::CanRegister).boolean().not_null().default(false))
                    .col(ColumnDef::new(TournamentConfig::CreatedAt).timestamp().not_null().default(Expr::current_timestamp()))
                    .col(ColumnDef::new(TournamentConfig::UpdatedAt).timestamp().not_null().default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await?;

        // 创建多人游戏房间表
        manager
            .create_table(
                Table::create()
                    .table(MultiplayerRoom::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MultiplayerRoom::RoomId)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MultiplayerRoom::Type).custom(MultiplayerRoomType::Table).not_null().default("SOLO"))
                    .col(ColumnDef::new(MultiplayerRoom::Season).custom(Season::Table).not_null().default("S1"))
                    .col(ColumnDef::new(MultiplayerRoom::Category).custom(Category::Table).not_null().default("QUA"))
                    .col(ColumnDef::new(MultiplayerRoom::MultiplayerSoloRoomRoomId).integer().null())
                    .col(ColumnDef::new(MultiplayerRoom::MultiplayerTeamvsRoomRoomId).integer().null())
                    .col(ColumnDef::new(MultiplayerRoom::ScoreRed).integer().not_null().default(0))
                    .col(ColumnDef::new(MultiplayerRoom::ScoreBlue).integer().not_null().default(0))
                    .col(ColumnDef::new(MultiplayerRoom::StartTime).timestamp().not_null())
                    .col(ColumnDef::new(MultiplayerRoom::EndTime).timestamp().not_null())
                    .col(ColumnDef::new(MultiplayerRoom::CreatedAt).timestamp().not_null().default(Expr::current_timestamp()))
                    .col(ColumnDef::new(MultiplayerRoom::UpdatedAt).timestamp().not_null().default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await?;

        // 创建单人游戏房间表
        manager
            .create_table(
                Table::create()
                    .table(MultiplayerSoloRoom::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MultiplayerSoloRoom::RoomId)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MultiplayerSoloRoom::PlayerRedId).integer().not_null().unique_key())
                    .col(ColumnDef::new(MultiplayerSoloRoom::PlayerBlueId).integer().not_null().unique_key())
                    .to_owned(),
            )
            .await?;

        // 创建团队对战房间表
        manager
            .create_table(
                Table::create()
                    .table(MultiplayerTeamvsRoom::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MultiplayerTeamvsRoom::RoomId)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MultiplayerTeamvsRoom::TeamRedId).integer().not_null().unique_key())
                    .col(ColumnDef::new(MultiplayerTeamvsRoom::TeamBlueId).integer().not_null().unique_key())
                    .to_owned(),
            )
            .await?;

        // 创建消息表
        manager
            .create_table(
                Table::create()
                    .table(Message::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Message::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Message::Title).string().not_null())
                    .col(ColumnDef::new(Message::Content).text().not_null())
                    .col(ColumnDef::new(Message::SenderId).integer().not_null())
                    .col(ColumnDef::new(Message::ReceiverId).integer().not_null())
                    .col(ColumnDef::new(Message::RoomId).integer().null())
                    .col(ColumnDef::new(Message::IsRead).boolean().not_null().default(false))
                    .col(ColumnDef::new(Message::IsDeleted).boolean().not_null().default(false))
                    .col(ColumnDef::new(Message::IsSystemMessage).boolean().not_null().default(false))
                    .col(ColumnDef::new(Message::IsAccepted).boolean().not_null().default(false))
                    .col(ColumnDef::new(Message::CreatedAt).timestamp().not_null().default(Expr::current_timestamp()))
                    .col(ColumnDef::new(Message::UpdatedAt).timestamp().not_null().default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await?;

        // 创建外键约束
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_multiplayer_room_solo_room")
                    .from(MultiplayerRoom::Table, MultiplayerRoom::MultiplayerSoloRoomRoomId)
                    .to(MultiplayerSoloRoom::Table, MultiplayerSoloRoom::RoomId)
                    .on_delete(ForeignKeyAction::SetNull)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_multiplayer_room_teamvs_room")
                    .from(MultiplayerRoom::Table, MultiplayerRoom::MultiplayerTeamvsRoomRoomId)
                    .to(MultiplayerTeamvsRoom::Table, MultiplayerTeamvsRoom::RoomId)
                    .on_delete(ForeignKeyAction::SetNull)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_multiplayer_solo_room_player_red")
                    .from(MultiplayerSoloRoom::Table, MultiplayerSoloRoom::PlayerRedId)
                    .to(User::Table, User::Osuid)
                    .on_delete(ForeignKeyAction::Restrict)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_multiplayer_solo_room_player_blue")
                    .from(MultiplayerSoloRoom::Table, MultiplayerSoloRoom::PlayerBlueId)
                    .to(User::Table, User::Osuid)
                    .on_delete(ForeignKeyAction::Restrict)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_multiplayer_teamvs_room_team_red")
                    .from(MultiplayerTeamvsRoom::Table, MultiplayerTeamvsRoom::TeamRedId)
                    .to(Team::Table, Team::Id)
                    .on_delete(ForeignKeyAction::Restrict)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_multiplayer_teamvs_room_team_blue")
                    .from(MultiplayerTeamvsRoom::Table, MultiplayerTeamvsRoom::TeamBlueId)
                    .to(Team::Table, Team::Id)
                    .on_delete(ForeignKeyAction::Restrict)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_message_sender")
                    .from(Message::Table, Message::SenderId)
                    .to(User::Table, User::Osuid)
                    .on_delete(ForeignKeyAction::Restrict)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_message_receiver")
                    .from(Message::Table, Message::ReceiverId)
                    .to(User::Table, User::Osuid)
                    .on_delete(ForeignKeyAction::Restrict)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_message_room")
                    .from(Message::Table, Message::RoomId)
                    .to(MultiplayerRoom::Table, MultiplayerRoom::RoomId)
                    .on_delete(ForeignKeyAction::SetNull)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 删除外键约束
        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_message_room")
                    .table(Message::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_message_receiver")
                    .table(Message::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_message_sender")
                    .table(Message::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_multiplayer_teamvs_room_team_blue")
                    .table(MultiplayerTeamvsRoom::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_multiplayer_teamvs_room_team_red")
                    .table(MultiplayerTeamvsRoom::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_multiplayer_solo_room_player_blue")
                    .table(MultiplayerSoloRoom::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_multiplayer_solo_room_player_red")
                    .table(MultiplayerSoloRoom::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_multiplayer_room_teamvs_room")
                    .table(MultiplayerRoom::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_multiplayer_room_solo_room")
                    .table(MultiplayerRoom::Table)
                    .to_owned(),
            )
            .await?;

        // 删除表
        manager
            .drop_table(Table::drop().table(Message::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(MultiplayerTeamvsRoom::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(MultiplayerSoloRoom::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(MultiplayerRoom::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(TournamentConfig::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Team::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(User::Table).to_owned())
            .await?;

        // 删除枚举类型
        manager
            .drop_type(Type::drop().name(Category::Table).to_owned())
            .await?;

        manager
            .drop_type(Type::drop().name(Season::Table).to_owned())
            .await?;

        manager
            .drop_type(Type::drop().name(UserGroupType::Table).to_owned())
            .await?;

        manager
            .drop_type(Type::drop().name(UserState::Table).to_owned())
            .await?;

        manager
            .drop_type(Type::drop().name(TeamState::Table).to_owned())
            .await?;

        manager
            .drop_type(Type::drop().name(TeamColor::Table).to_owned())
            .await?;

        manager
            .drop_type(Type::drop().name(MultiplayerRoomType::Table).to_owned())
            .await?;

        manager
            .drop_type(Type::drop().name(BeatmapSelectMod::Table).to_owned())
            .await?;

        Ok(())
    }
}

// 定义表名枚举
#[derive(Iden)]
enum User {
    Table,
    Id,
    UserState,
    Osuid,
    Username,
    AvatarUrl,
    CoverUrl,
    CountryCode,
    Pp,
    GlobalRank,
    CountryRank,
    Approved,
    Seed,
    Seasonal,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Team {
    Table,
    Id,
    Name,
    BannerUrl,
    TeamColor,
    TeamState,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum TournamentConfig {
    Table,
    Id,
    TournamentName,
    MaxPpForRegistration,
    MinPpForRegistration,
    CurrentSeasonal,
    CurrentCategory,
    CanRegister,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum MultiplayerRoom {
    Table,
    RoomId,
    Type,
    Season,
    Category,
    MultiplayerSoloRoomRoomId,
    MultiplayerTeamvsRoomRoomId,
    ScoreRed,
    ScoreBlue,
    StartTime,
    EndTime,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum MultiplayerSoloRoom {
    Table,
    RoomId,
    PlayerRedId,
    PlayerBlueId,
}

#[derive(Iden)]
enum MultiplayerTeamvsRoom {
    Table,
    RoomId,
    TeamRedId,
    TeamBlueId,
}

#[derive(Iden)]
enum Message {
    Table,
    Id,
    Title,
    Content,
    SenderId,
    ReceiverId,
    RoomId,
    IsRead,
    IsDeleted,
    IsSystemMessage,
    IsAccepted,
    CreatedAt,
    UpdatedAt,
}

// 定义枚举类型名
#[derive(Iden)]
enum BeatmapSelectMod {
    Table,
    Nm,
    Hd,
    Hr,
    Dt,
    Lz,
    Tb,
}

#[derive(Iden)]
enum MultiplayerRoomType {
    Table,
    Solo,
    TeamVs,
}

#[derive(Iden)]
enum TeamColor {
    Table,
    BlueTeam,
    RedTeam,
}

#[derive(Iden)]
enum TeamState {
    Table,
    Active,
    Unapproved,
    Approved,
    Banned,
}

#[derive(Iden)]
enum UserState {
    Table,
    Active,
    Registered,
    Abandoned,
    Banned,
}

#[derive(Iden)]
enum UserGroupType {
    Table,
    Host,
    Admin,
    Pooler,
    Streamer,
    Tester,
    Gfx,
    Sheeter,
    Commentator,
    Mapper,
    Referee,
    Player,
}

#[derive(Iden)]
enum Season {
    Table,
    S1,
    S2,
}

#[derive(Iden)]
enum Category {
    Table,
    Qua,
    Ro16,
    Qf,
    Sf,
    F,
    Gf,
}
