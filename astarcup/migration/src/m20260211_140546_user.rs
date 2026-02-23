use sea_orm_migration::{prelude::*, schema::*};
use sea_query::extension::postgres::Type;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_type(
            Type::create()
                .as_enum(Alias::new("userstate"))
                .values([Alias::new("active"),Alias::new("approved"),Alias::new("banned"),Alias::new("ignored")])
                .to_owned(),
            )
            .await?;
        manager
            .create_type(
                Type::create()
                    .as_enum(Alias::new("usergroup"))
                    .values([Alias::new("player"),Alias::new("admin"),Alias::new("mapooler"),Alias::new("tester")])
                    .to_owned(),
            )
            .await?;
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
                    .col(
                        ColumnDef::new(User::OsuId)
                            .integer()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(User::Username)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(User::AvatarUrl).string().null())
                    .col(ColumnDef::new(User::CoverUrl).string().null())
                    .col(ColumnDef::new(User::Pp).float().null())
                    .col(ColumnDef::new(User::GlobalRank).integer().null())
                    .col(ColumnDef::new(User::CountryCode).string().null())
                    .col(ColumnDef::new(User::CountryRank).integer().null())
                    .col(
                        ColumnDef::new(User::UserState)
                            .custom(Alias::new("userstate"))
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        ColumnDef::new(User::UserGroup)
                            .custom(Alias::new("usergroup"))
                            .not_null()
                            .default("player"),
                    )
                    .col(ColumnDef::new(User::Season).integer().null())
                    .col(ColumnDef::new(User::GuServerUserId).integer().null())
                    .col(ColumnDef::new(User::GuServerUsername).string().null())
                    .col(ColumnDef::new(User::GuServerBindAt).timestamp().null())
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
        manager.drop_type(Type::drop().name(Alias::new("userstate")).to_owned()).await?;
        manager.drop_type(Type::drop().name(Alias::new("usergroup")).to_owned()).await?;
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
    CoverUrl,
    AvatarUrl,
    Pp,
    GlobalRank,
    CountryCode,
    CountryRank,
    UserState,
    UserGroup,
    Season,
    GuServerUserId,
    GuServerUsername,
    GuServerBindAt,
    CreatedAt,
    UpdatedAt,
}
