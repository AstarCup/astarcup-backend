use sea_orm::prelude::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};

use crate::{
    dto::map_selections::{
        CreateMapSelectionRequest, MapSelectionQueryParams, UpdateMapSelectionRequest,
    },
    entity::map_selections,
    error::{AppError, Result},
};

pub struct MapSelectionService {
    db: DatabaseConnection,
}

impl MapSelectionService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn find_all(
        &self,
        params: MapSelectionQueryParams,
    ) -> Result<(Vec<map_selections::Model>, u64)> {
        let mut query = map_selections::Entity::find();

        if let Some(season) = params.season {
            query = query.filter(map_selections::Column::Season.eq(season));
        }

        if let Some(category) = params.category {
            query = query.filter(map_selections::Column::Category.eq(category));
        }

        if let Some(selected_by) = params.selected_by {
            query = query.filter(map_selections::Column::SelectedBy.eq(selected_by));
        }

        query = query.order_by_desc(map_selections::Column::CreatedAt);

        let page = params.page.unwrap_or(1);
        let page_size = params.page_size.unwrap_or(20);

        let total = query.clone().count(&self.db).await?;
        let map_selections = query
            .paginate(&self.db, page_size)
            .fetch_page(page - 1)
            .await?;

        Ok((map_selections, total))
    }

    pub async fn find_by_id(&self, id: i32) -> Result<map_selections::Model> {
        map_selections::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("地图选择 {} 不存在", id)))
    }

    pub async fn create(&self, data: CreateMapSelectionRequest) -> Result<map_selections::Model> {
        let now = chrono::Utc::now();

        let map_selection = map_selections::ActiveModel {
            beatmap_id: Set(data.beatmap_id),
            beatmapset_id: Set(data.beatmapset_id),
            title: Set(data.title),
            title_unicode: Set(data.title_unicode),
            artist: Set(data.artist),
            artist_unicode: Set(data.artist_unicode),
            version: Set(data.version),
            creator: Set(data.creator),
            star_rating: Set(Decimal::from(data.star_rating.parse::<i64>().unwrap())),
            bpm: Set(Decimal::from(data.bpm.parse::<i64>().unwrap())),
            total_length: Set(data.total_length),
            max_combo: Set(data.max_combo),
            ar: Set(Decimal::from(data.ar.parse::<i64>().unwrap())),
            cs: Set(Decimal::from(data.cs.parse::<i64>().unwrap())),
            od: Set(Decimal::from(data.od.parse::<i64>().unwrap())),
            hp: Set(Decimal::from(data.hp.parse::<i64>().unwrap())),
            selected_mods: Set(data.selected_mods),
            mod_position: Set(data.mod_position),
            comment: Set(data.comment),
            selected_by: Set(data.selected_by),
            selected_at: Set(now),
            season: Set(data.season),
            category: Set(data.category),
            url: Set(data.url),
            cover_url: Set(data.cover_url),
            approved: Set(data.approved),
            padding: Set(data.padding),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        Ok(map_selection.insert(&self.db).await?)
    }

    pub async fn update(
        &self,
        id: i32,
        data: UpdateMapSelectionRequest,
    ) -> Result<map_selections::Model> {
        let mut map_selection: map_selections::ActiveModel = self.find_by_id(id).await?.into();

        if let Some(title) = data.title {
            map_selection.title = Set(title);
        }

        if let Some(title_unicode) = data.title_unicode {
            map_selection.title_unicode = Set(Some(title_unicode));
        }

        if let Some(artist) = data.artist {
            map_selection.artist = Set(artist);
        }

        if let Some(artist_unicode) = data.artist_unicode {
            map_selection.artist_unicode = Set(Some(artist_unicode));
        }

        if let Some(version) = data.version {
            map_selection.version = Set(version);
        }

        if let Some(creator) = data.creator {
            map_selection.creator = Set(creator);
        }

        if let Some(star_rating) = data.star_rating {
            map_selection.star_rating = Set(Decimal::from(star_rating.parse::<i64>().unwrap()));
        }

        if let Some(bpm) = data.bpm {
            map_selection.bpm = Set(Decimal::from(bpm.parse::<i64>().unwrap()));
        }

        if let Some(total_length) = data.total_length {
            map_selection.total_length = Set(total_length);
        }

        if let Some(max_combo) = data.max_combo {
            map_selection.max_combo = Set(max_combo);
        }

        if let Some(ar) = data.ar {
            map_selection.ar = Set(Decimal::from(ar.parse::<i64>().unwrap()));
        }

        if let Some(cs) = data.cs {
            map_selection.cs = Set(Decimal::from(cs.parse::<i64>().unwrap()));
        }

        if let Some(od) = data.od {
            map_selection.od = Set(Decimal::from(od.parse::<i64>().unwrap()));
        }

        if let Some(hp) = data.hp {
            map_selection.hp = Set(Decimal::from(hp.parse::<i64>().unwrap()));
        }

        if let Some(selected_mods) = data.selected_mods {
            map_selection.selected_mods = Set(selected_mods);
        }

        if let Some(mod_position) = data.mod_position {
            map_selection.mod_position = Set(mod_position);
        }

        if let Some(comment) = data.comment {
            map_selection.comment = Set(Some(comment));
        }

        if let Some(season) = data.season {
            map_selection.season = Set(season);
        }

        if let Some(category) = data.category {
            map_selection.category = Set(category);
        }

        if let Some(url) = data.url {
            map_selection.url = Set(url);
        }

        if let Some(cover_url) = data.cover_url {
            map_selection.cover_url = Set(Some(cover_url));
        }

        if let Some(approved) = data.approved {
            map_selection.approved = Set(Some(approved));
        }

        if let Some(padding) = data.padding {
            map_selection.padding = Set(Some(padding));
        }

        map_selection.updated_at = Set(chrono::Utc::now());

        Ok(map_selection.update(&self.db).await?)
    }

    pub async fn delete(&self, id: i32) -> Result<()> {
        let result = map_selections::Entity::delete_by_id(id)
            .exec(&self.db)
            .await?;
        if result.rows_affected == 0 {
            return Err(AppError::NotFound(format!("地图选择 {} 不存在", id)));
        }
        Ok(())
    }
}
