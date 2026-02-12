use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
                    .col(ColumnDef::new(Messages::SenderUsername).string().not_null())
                    .col(ColumnDef::new(Messages::ReceiverOsuId).string().not_null())
                    .col(
                        ColumnDef::new(Messages::ReceiverUsername)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Messages::Type)
                            .enumeration(
                                "type",
                                vec!["match_invitation", "match_response", "system"],
                            )
                            .null(),
                    )
                    .col(ColumnDef::new(Messages::Title).string().not_null())
                    .col(ColumnDef::new(Messages::Content).text().not_null())
                    .col(ColumnDef::new(Messages::RelatedMatchupId).integer().null())
                    .col(
                        ColumnDef::new(Messages::Status)
                            .enumeration("status", vec!["available", "scheduled", "completed"])
                            .null(),
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
            .drop_table(Table::drop().table(Messages::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Messages {
    Table,
    Id,
    SenderOsuId,
    SenderUsername,
    ReceiverOsuId,
    ReceiverUsername,
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
