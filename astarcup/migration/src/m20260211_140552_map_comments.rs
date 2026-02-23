use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MapComments::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MapComments::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MapComments::MapSelectionId)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(MapComments::OsuId).string().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(MapComments::Table, MapComments::OsuId)
                            .to(User::Table, User::OsuId),
                    )
                    .col(ColumnDef::new(MapComments::Comment).text().null())
                    .col(
                        ColumnDef::new(MapComments::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MapComments::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(MapComments::Table, MapComments::MapSelectionId)
                            .to(MapSelections::Table, MapSelections::Id),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MapComments::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum MapComments {
    Table,
    Id,
    MapSelectionId,
    OsuId,
    Comment,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum MapSelections {
    Table,
    Id,
}
#[derive(DeriveIden)]
enum User {
    Table,
    OsuId,
}
