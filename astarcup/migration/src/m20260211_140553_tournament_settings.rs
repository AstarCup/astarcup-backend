use sea_orm_migration::prelude::*;
use crate::extension::postgres::Type;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_type(
                Type::create()
                    .as_enum(Alias::new("tournament_setting_key"))
                    .values([
                        Alias::new("TournamentName"),
                        Alias::new("MaxPpForRegistration"),
                        Alias::new("MinPpForRegistration"),
                        Alias::new("CurrentSeason"),
                        Alias::new("CurrentCategory"),
                        Alias::new("MapPoolVisible"),
                        Alias::new("CanRegister")
                    ])
                    .to_owned(),
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
                                    "MapPoolVisible",
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
            .drop_table(Table::drop().table(TournamentSettings::Table).to_owned())
            .await?;
        manager
            .drop_type(
            Type::drop().name(Alias::new("tournament_setting_key")).to_owned(),
            )
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
