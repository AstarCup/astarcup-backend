use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
                    .col(ColumnDef::new(User::OsuId).string().not_null().unique_key())
                    .col(
                        ColumnDef::new(User::Username)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(User::RegisteredAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(User::AvatarUrl).string().null())
                    .col(ColumnDef::new(User::Pp).float().null())
                    .col(ColumnDef::new(User::GlobalRank).integer().null())
                    .col(ColumnDef::new(User::Country).string().null())
                    .col(ColumnDef::new(User::CountryRank).integer().null())
                    .col(ColumnDef::new(User::Approved).integer().null())
                    .col(
                        ColumnDef::new(User::UserGroup)
                            .enumeration("usergroup", vec!["player", "admin"])
                            .not_null()
                            .default("player"),
                    )
                    .col(ColumnDef::new(User::Season).integer().null())
                    .col(ColumnDef::new(User::GuServerUserId).integer().null())
                    .col(ColumnDef::new(User::GuServerUsername).string().null())
                    .col(
                        ColumnDef::new(User::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(User::UpdatedAt)
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
            .drop_table(Table::drop().table(User::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum User {
    Table,
    Id,
    OsuId,
    Username,
    RegisteredAt,
    AvatarUrl,
    Pp,
    GlobalRank,
    Country,
    CountryRank,
    Approved,
    UserGroup,
    Season,
    GuServerUserId,
    GuServerUsername,
    CreatedAt,
    UpdatedAt,
}
