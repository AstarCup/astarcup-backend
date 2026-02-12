use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MatchSchedules::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MatchSchedules::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MatchSchedules::RoomId).integer().not_null())
                    .col(
                        ColumnDef::new(MatchSchedules::Player1OsuId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MatchSchedules::Player1Username)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MatchSchedules::Player2OsuId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MatchSchedules::Player2Username)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(MatchSchedules::RedScore).integer().null())
                    .col(ColumnDef::new(MatchSchedules::BlueScore).integer().null())
                    .col(
                        ColumnDef::new(MatchSchedules::Status)
                            .enumeration("status", vec!["available", "scheduled", "completed"])
                            .null(),
                    )
                    .col(ColumnDef::new(MatchSchedules::ReplayLink).text().null())
                    .col(ColumnDef::new(MatchSchedules::MatchLink).text().null())
                    .col(
                        ColumnDef::new(MatchSchedules::CreatedBy)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MatchSchedules::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MatchSchedules::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(MatchSchedules::Table, MatchSchedules::RoomId)
                            .to(MatchRooms::Table, MatchRooms::Id),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MatchSchedules::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum MatchSchedules {
    Table,
    Id,
    RoomId,
    Player1OsuId,
    Player1Username,
    Player2OsuId,
    Player2Username,
    RedScore,
    BlueScore,
    Status,
    ReplayLink,
    MatchLink,
    CreatedBy,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum MatchRooms {
    Table,
    Id,
}
