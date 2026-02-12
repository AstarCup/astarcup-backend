use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
                        ColumnDef::new(TournamentSettings::TournamentName)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(TournamentSettings::MaxPpForRegistration).float().null())
                    .col(ColumnDef::new(TournamentSettings::MinPpForRegistration).float().null())
                    .col(ColumnDef::new(TournamentSettings::CurrentSeason).string().null())
                    .col(ColumnDef::new(TournamentSettings::CurrentSeasonStage).string().null())
                    .col(ColumnDef::new(TournamentSettings::MappoolVisible).integer().null())
                    .col(
                        ColumnDef::new(TournamentSettings::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(TournamentSettings::UpdatedAt)
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
            .drop_table(Table::drop().table(TournamentSettings::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum TournamentSettings {
    Table,
    Id,
    TournamentName,
    MaxPpForRegistration,
    MinPpForRegistration,
    CurrentSeason,
    CurrentSeasonStage,
    MappoolVisible,
    CreatedAt,
    UpdatedAt,
}
