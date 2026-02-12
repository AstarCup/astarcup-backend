use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};

use crate::{
    dto::map_comments::{CreateMapCommentRequest, MapCommentQueryParams, UpdateMapCommentRequest},
    entity::map_comments,
    error::{AppError, Result},
};

pub struct MapCommentService {
    db: DatabaseConnection,
}

impl MapCommentService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn find_all(
        &self,
        params: MapCommentQueryParams,
    ) -> Result<(Vec<map_comments::Model>, u64)> {
        let mut query = map_comments::Entity::find();

        if let Some(map_selection_id) = params.map_selection_id {
            query = query.filter(map_comments::Column::MapSelectionId.eq(map_selection_id));
        }

        if let Some(osu_id) = params.osu_id {
            query = query.filter(map_comments::Column::OsuId.eq(osu_id));
        }

        query = query.order_by_desc(map_comments::Column::CreatedAt);

        let page = params.page.unwrap_or(1);
        let page_size = params.page_size.unwrap_or(20);

        let total = query.clone().count(&self.db).await?;
        let map_comments = query
            .paginate(&self.db, page_size)
            .fetch_page(page - 1)
            .await?;

        Ok((map_comments, total))
    }

    pub async fn find_by_id(&self, id: i32) -> Result<map_comments::Model> {
        map_comments::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("地图评论 {} 不存在", id)))
    }

    pub async fn create(&self, data: CreateMapCommentRequest) -> Result<map_comments::Model> {
        let now = chrono::Utc::now();

        let map_comment = map_comments::ActiveModel {
            map_selection_id: Set(data.map_selection_id),
            osu_id: Set(data.osu_id),
            username: Set(data.username),
            comment: Set(data.comment),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        Ok(map_comment.insert(&self.db).await?)
    }

    pub async fn update(
        &self,
        id: i32,
        data: UpdateMapCommentRequest,
    ) -> Result<map_comments::Model> {
        let mut map_comment: map_comments::ActiveModel = self.find_by_id(id).await?.into();

        if let Some(comment) = data.comment {
            map_comment.comment = Set(Some(comment));
        }

        map_comment.updated_at = Set(chrono::Utc::now());

        Ok(map_comment.update(&self.db).await?)
    }

    pub async fn delete(&self, id: i32) -> Result<()> {
        let result = map_comments::Entity::delete_by_id(id)
            .exec(&self.db)
            .await?;
        if result.rows_affected == 0 {
            return Err(AppError::NotFound(format!("地图评论 {} 不存在", id)));
        }
        Ok(())
    }
}
