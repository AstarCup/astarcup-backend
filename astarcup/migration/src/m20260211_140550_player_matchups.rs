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
                        ColumnDef::new(PlayerMatchups::Season)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(PlayerMatchups::Category)
                            .custom(Alias::new("category"))
                            .default("qua")
                            .not_null()
                    )
                    .col(
                        ColumnDef::new(PlayerMatchups::PlayerRedOsuId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerMatchups::PlayerBlueOsuId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerMatchups::Status)
                            .custom(Alias::new("matchStatus"))
                            .default("available")
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
            .drop_type(
                Type::drop().name(Alias::new("category")).if_exists().to_owned(),
            )
            .await?;
        manager
            .drop_type(
                Type::drop().name(Alias::new("matchStatus")).if_exists().to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(PlayerMatchups::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum PlayerMatchups {
    Table,
    Id,
    Season,
    Category,
    PlayerRedOsuId,
    PlayerBlueOsuId,
    Status,
    CreatedAt,
    UpdatedAt,
}
