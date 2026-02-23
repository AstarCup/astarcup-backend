use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_type()
            .create_enum("category", vec!["qua", "ro16", "qf", "sf", "f", "gf"])
            .await?;
        manager
            .create_type()
            .create_enum("status", vec!["available", "scheduled", "completed"])
            .await?;
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
                            .enumeration("category", vec!["qua", "ro16", "qf", "sf", "f", "gf"])
                            .not_null()
                            .default("qua"),
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
                            .enumeration("status", vec!["available", "scheduled", "completed"])
                            .null(),
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
                sea_orm_migration::prelude::Enum::drop()
                    .enum_name("category")
                    .to_owned(),
            )
            .await?;
        manager
            .drop_type(
                sea_orm_migration::prelude::Enum::drop()
                    .enum_name("status")
                    .to_owned(),
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
