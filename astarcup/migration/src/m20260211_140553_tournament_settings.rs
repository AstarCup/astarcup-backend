use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_type()
            .create_enum(
                "tournament_setting_key",
                vec![
                    "TournamentName",
                    "MaxPpForRegistration",
                    "MinPpForRegistration",
                    "CurrentSeason",
                    "CurrentSeasonStage",
                    "MappoolVisible",
                ],
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(TournamentSettings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TournamentSettings::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(TournamentSettings::Key)
                            .enumeration(
                                "tournament_setting_key",
                                vec![
                                    "TournamentName",
                                    "MaxPpForRegistration",
                                    "MinPpForRegistration",
                                    "CurrentSeason",
                                    "CurrentSeasonStage",
                                    "MappoolVisible",
                                ],
                            )
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TournamentSettings::Value)
                            .string()
                            .not_null(),
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
                    .enum_name("tournament_setting_key")
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(TournamentSettings::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum TournamentSettings {
    Table,
    Id,
    Key,
    Value,
}
