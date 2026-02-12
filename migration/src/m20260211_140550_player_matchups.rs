use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PlayerMatchups::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlayerMatchups::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PlayerMatchups::Player1OsuId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerMatchups::Player1Username)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerMatchups::Player2OsuId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerMatchups::Player2Username)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerMatchups::Status)
                            .enumeration("status", vec!["available", "scheduled", "completed"])
                            .null(),
                    )
                    .col(
                        ColumnDef::new(PlayerMatchups::CreatedBy)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerMatchups::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(PlayerMatchups::UpdatedAt)
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
        manager
            .drop_table(Table::drop().table(PlayerMatchups::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum PlayerMatchups {
    Table,
    Id,
    Player1OsuId,
    Player1Username,
    Player2OsuId,
    Player2Username,
    Status,
    CreatedBy,
    CreatedAt,
    UpdatedAt,
}
