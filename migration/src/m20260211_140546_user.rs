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
                    .col(
                        ColumnDef::new(User::UserState)
                            .enumeration("userstate", vec!["active", "banned", "deleted"])
                            .not_null()
                            .default(UserState::Active),
                    )
                    .col(ColumnDef::new(User::Pp).float().not_null().default(0.0))
                    .col(ColumnDef::new(User::CountryCode).string())
                    .col(ColumnDef::new(User::GlobalRank).integer().null())
                    .col(ColumnDef::new(User::CountryRank).integer().null())
                    .col(ColumnDef::new(User::Level).integer().not_null().default(1))
                    .col(
                        ColumnDef::new(User::RegisteredSeasonal)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    // gu服绑定
                    .col(
                        ColumnDef::new(User::GuServerId)
                            .integer()
                            .null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(User::GuServerUsername).string().null())
                    // 用户组
                    .col(
                        ColumnDef::new(User::UserGroupType)
                            .enumeration("usergrouptype", vec!["player", "admin"])
                            .not_null()
                            .default(UserGroupType::Player),
                    )
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
    AvatarUrl,
    CoverUrl,
    UserState,
    CountryCode,
    CountryRank,
    RegisteredSeasonal,
    GuServerId,
    GuServerUsername,
    UserGroupType,
    CreatedAt,
    UpdatedAt,
    Level,
    Pp,
    GlobalRank,
}

#[derive(DeriveIden)]
enum UserState {
    Table,
    Active,
    Banned,
    Deleted,
}

#[derive(DeriveIden)]
enum UserGroupType {
    Table,
    Player,
    Admin,
}

impl Into<Expr> for UserState {
    fn into(self) -> Expr {
        match self {
            UserState::Active => Expr::val("active"),
            UserState::Banned => Expr::val("banned"),
            UserState::Deleted => Expr::val("deleted"),
            UserState::Table => Expr::val("userstate"),
        }
    }
}

impl Into<Expr> for UserGroupType {
    fn into(self) -> Expr {
        match self {
            UserGroupType::Player => Expr::val("player"),
            UserGroupType::Admin => Expr::val("admin"),
            UserGroupType::Table => Expr::val("usergrouptype"),
        }
    }
}
