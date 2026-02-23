use sea_orm_migration::prelude::*;
use crate::extension::postgres::Type;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MatchRooms::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MatchRooms::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MatchRooms::RoundNumber).integer().not_null())
                    .col(ColumnDef::new(MatchRooms::MatchTime).timestamp().not_null())
                    .col(ColumnDef::new(MatchRooms::MatchNumber).integer().not_null())
                    .col(
                        ColumnDef::new(MatchRooms::Status)
                            .custom(Alias::new("match_status"))
                            .default("available")
                            .null(),
                    )
                    .col(ColumnDef::new(MatchRooms::CreatedBy).string().not_null())
                    .col(
                        ColumnDef::new(MatchRooms::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MatchRooms::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_type(Type::drop().name(Alias::new("match_status")).to_owned()).await?;
        manager
            .drop_table(Table::drop().table(MatchRooms::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum MatchRooms {
    Table,
    Id,
    RoundNumber,
    MatchTime,
    MatchNumber,
    Status,
    CreatedBy,
    CreatedAt,
    UpdatedAt,
}
