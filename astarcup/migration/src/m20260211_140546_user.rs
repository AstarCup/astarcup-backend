use sea_orm_migration::prelude::*;
use sea_query::extension::postgres::Type;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_type(
            Type::create()
                .as_enum(Alias::new("user_state"))
                .values([Alias::new("active"),Alias::new("approved"),Alias::new("banned"),Alias::new("ignored")])
                .to_owned(),
            )
            .await?;
        manager
            .create_type(
                Type::create()
                    .as_enum(Alias::new("user_group"))
                    .values([Alias::new("player"),Alias::new("admin"),Alias::new("mapooler"),Alias::new("tester")])
                    .to_owned(),
            )
            .await?;
        manager
            .create_type(
                Type::create()
                    .as_enum(Alias::new("match_status"))
                    .values([Alias::new("available"), Alias::new("scheduled"), Alias::new("completed")])
                    .to_owned(),
            )
            .await?;
        manager
            .create_type(
                Type::create()
                    .as_enum(Alias::new("category"))
                    .values([Alias::new("qua"), Alias::new("ro16"), Alias::new("qf"),Alias::new("sf"),Alias::new("f"),Alias::new("gf")])
                    .to_owned(),
            )
            .await?;
        manager
            .create_type(
                Type::create()
                    .as_enum(Alias::new("mode"))
                    .values([Alias::new("std"),Alias::new("taiko"),Alias::new("ctb"),Alias::new("mania")])
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
                            .custom(Alias::new("user_state"))
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        ColumnDef::new(User::UserGroup)
                            .custom(Alias::new("user_group"))
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
        manager.drop_type(Type::drop().name(Alias::new("user_state")).to_owned()).await?;
        manager.drop_type(Type::drop().name(Alias::new("user_group")).to_owned()).await?;
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
