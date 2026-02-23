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
                    .as_enum(Alias::new("type"))
                    .values([Alias::new("match_invitation"), Alias::new("match_response"), Alias::new("system")])
                    .to_owned(),
            )
            .await?;
        manager
            .create_type(
                Type::create()
                    .as_enum(Alias::new("status"))
                    .values([Alias::new("available"),Alias::new("scheduled"),Alias::new("completed")])
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Messages::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Messages::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Messages::SenderOsuId).string().not_null())
                    .col(ColumnDef::new(Messages::ReceiverOsuId).string().not_null())
                    .col(
                        ColumnDef::new(Messages::Type)
                            .custom(Alias::new("type"))
                            .default("match_invitation")
                            .not_null(),
                    )
                    .col(ColumnDef::new(Messages::Title).string().not_null())
                    .col(ColumnDef::new(Messages::Content).text().not_null())
                    .col(ColumnDef::new(Messages::RelatedMatchupId).integer().null())
                    .col(
                        ColumnDef::new(Messages::Status)
                            .custom(Alias::new("match_status"))
                            .default("available")
                            .not_null(),
                    )
                    .col(ColumnDef::new(Messages::ResponseAction).string().null())
                    .col(ColumnDef::new(Messages::ResponseTime).timestamp().null())
                    .col(
                        ColumnDef::new(Messages::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Messages::UpdatedAt)
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
               Type::drop().name(Alias::new("type")).to_owned(),
            )
            .await?;
        manager
            .drop_type(
                Type::drop().name(Alias::new("match_status")).to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Messages::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Messages {
    Table,
    Id,
    SenderOsuId,
    ReceiverOsuId,
    Type,
    Title,
    Content,
    RelatedMatchupId,
    Status,
    ResponseAction,
    ResponseTime,
    CreatedAt,
    UpdatedAt,
}
