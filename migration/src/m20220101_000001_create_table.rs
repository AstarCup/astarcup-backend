use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        todo!();
        // type creation

        /// 用户类型
        manager
            .create_type(
                Type::create()
                    .as_enum(UserState::Table)
                    .values([
                        UserState::Active,
                        UserState::Registered,
                        UserState::Banned,
                        UserState::Deleted,
                    ])
                    .to_owned(),
            )
            .await?;
        /// 用户组
        manager
            .create_type(
                Type::create()
                    .as_enum(UserGroupType::Table)
                    .values([
                        UserGroupType::Player,
                        UserGroupType::Admin,
                        UserGroupType::Pooler,
                        UserGroupType::Tester,
                    ])
                    .to_owned(),
            )
            .await?;

        /// 比赛阶段
        manager
            .create_type(
                Type::create()
                    .as_enum(SeasonState::Table)
                    .values([
                        SeasonState::Qua,
                        SeasonState::Ro16,
                        SeasonState::Qf,
                        SeasonState::Sf,
                        SeasonState::F,
                        SeasonState::Gf,
                    ])
                    .to_owned(),
            )
            .await?;

        /// 比赛状态
        manager
            .create_type(
                type_::create()
                    .as_enum(MatchState::Table)
                    .values([
                        MatchState::Waiting,
                        MatchState::Running,
                        MatchState::Finished,
                    ])
                    .to_owned(),
            )
            .await?;

        // table creation

        /// user table
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
                        ColumnDef::new(User::Osuid)
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
                            .custom(UserState::Table)
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
                            .custom(UserGroupType::Table)
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

        /// schedule table
        manager
            .create_table(
                Table::create()
                    .table(Schedule::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Schedule::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Schedule::Season)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(Schedule::Category)
                            .custom(Category::Table)
                            .not_null()
                            .default(Category::Qua),
                    )
                    .col(ColumnDef::new(Schedule::RedPlayerId).integer().null())
                    .col(ColumnDef::new(Schedule::BluePlayerId).integer().null())
                    .col(ColumnDef::new(Schedule::RedScore).integer().null())
                    .col(ColumnDef::new(Schedule::BlueScore).integer().null())
                    .col(
                        ColumnDef::new(Schedule::State)
                            .custom(MatchState::Table)
                            .not_null()
                            .default(MatchState::Waiting),
                    ),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        todo!();

        // table deletion
        manager
            .drop_table(Table::drop().table(User::Table).to_owned())
            .await
    }
}
