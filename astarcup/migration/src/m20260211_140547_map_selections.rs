use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MapSelections::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MapSelections::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MapSelections::BeatmapId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MapSelections::BeatmapSetId)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(MapSelections::Title).string().null())
                    .col(ColumnDef::new(MapSelections::TitleUnicode).string().null())
                    .col(ColumnDef::new(MapSelections::Artist).string().null())
                    .col(ColumnDef::new(MapSelections::ArtistUnicode).string().null())
                    .col(ColumnDef::new(MapSelections::Version).string().null())
                    .col(ColumnDef::new(MapSelections::Creator).string().null())
                    .col(ColumnDef::new(MapSelections::StarRating).decimal().null())
                    .col(ColumnDef::new(MapSelections::Bpm).decimal().null())
                    .col(ColumnDef::new(MapSelections::TotalLength).integer().null())
                    .col(ColumnDef::new(MapSelections::MaxCombo).integer().null())
                    .col(ColumnDef::new(MapSelections::Ar).decimal().null())
                    .col(ColumnDef::new(MapSelections::Cs).decimal().null())
                    .col(ColumnDef::new(MapSelections::Od).decimal().null())
                    .col(ColumnDef::new(MapSelections::Hp).decimal().null())
                    .col(ColumnDef::new(MapSelections::SelectedMods).json().null())
                    .col(
                        ColumnDef::new(MapSelections::SelectedModPosition)
                            .integer()
                            .null(),
                    )
                    .col(ColumnDef::new(MapSelections::Comment).text().null())
                    .col(ColumnDef::new(MapSelections::SelectedBy).integer().null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(MapSelections::Table, MapSelections::SelectedBy)
                            .to(Users::Table, Users::Id),
                    )
                    .col(
                        ColumnDef::new(MapSelections::SelectedAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(ColumnDef::new(MapSelections::Season).string().not_null())
                    .col(ColumnDef::new(MapSelections::Category).string().not_null())
                    .col(ColumnDef::new(MapSelections::CoverUrl).text().null())
                    .col(
                        ColumnDef::new(MapSelections::IsApproved)
                            .boolean()
                            .null()
                            .default(Expr::bool(false)),
                    )
                    .col(
                        ColumnDef::new(MapSelections::IsNeedTest)
                            .boolean()
                            .null()
                            .default(Expr::bool(false)),
                    )
                    .col(
                        ColumnDef::new(MapSelections::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MapSelections::UpdatedAt)
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
            .drop_table(Table::drop().table(MapSelections::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum MapSelections {
    Table,
    Id,
    BeatmapId,
    BeatmapsetId,
    Title,
    TitleUnicode,
    Artist,
    ArtistUnicode,
    Version,
    Creator,
    StarRating,
    Bpm,
    TotalLength,
    MaxCombo,
    Ar,
    Cs,
    Od,
    Hp,
    SelectedMods,
    ModPosition,
    Comment,
    SelectedBy,
    SelectedAt,
    Season,
    Category,
    Url,
    CoverUrl,
    Approved,
    Padding,
    CreatedAt,
    UpdatedAt,
}
